use algoram_core::Graph;
use algoram_interop::{Connector, RouteRegistry};
use algoram_runtime::{
    ExecutionAccessReport, ExecutionPlan, ExecutionPolicy, ExecutionTrace, GuardedProcessRuntime,
    ImplementationRegistry, Planner, ProcessAction,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::error::Error;
use std::fmt;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

pub const BRIDGE_CONFIG_SCHEMA_VERSION: &str = "algoram.runtime-bridge/0.1";
pub const MAX_HTTP_BODY_BYTES: usize = 8 * 1024 * 1024;
const MAX_HTTP_HEADER_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustedImplementationConfig {
    pub implementation_ref: String,
    pub action: ProcessAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeBridgeConfig {
    pub schema_version: String,
    #[serde(default)]
    pub implementations: Vec<TrustedImplementationConfig>,
    #[serde(default)]
    pub connectors: Vec<Connector>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanRequest {
    pub graph: Graph,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanResponse {
    pub plan: ExecutionPlan,
    pub access_report: ExecutionAccessReport,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunRequest {
    pub graph: Graph,
    pub expected_plan: ExecutionPlan,
    #[serde(default)]
    pub allowed_implementation_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunResponse {
    pub access_report: ExecutionAccessReport,
    pub trace: ExecutionTrace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthResponse {
    pub schema_version: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

#[derive(Debug)]
pub enum BridgeError {
    UnsupportedConfigVersion(String),
    EmptyImplementationRef,
    InvalidImplementation(String),
    InvalidConnector(String),
    InvalidGraph(String),
    Planning(String),
    NoExecutableSteps,
    Inspection(String),
    PlanChanged,
    Security(String),
    NonLoopbackBind(IpAddr),
    EmptyBearerToken,
    InvalidAllowedOrigin,
    Http(String),
    Json(serde_json::Error),
    Io(std::io::Error),
}

impl BridgeError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedConfigVersion(_) => "unsupported_config_version",
            Self::EmptyImplementationRef => "empty_implementation_ref",
            Self::InvalidImplementation(_) => "invalid_implementation",
            Self::InvalidConnector(_) => "invalid_connector",
            Self::InvalidGraph(_) => "invalid_graph",
            Self::Planning(_) => "planning_failed",
            Self::NoExecutableSteps => "no_executable_steps",
            Self::Inspection(_) => "access_inspection_failed",
            Self::PlanChanged => "plan_changed",
            Self::Security(_) => "execution_denied",
            Self::NonLoopbackBind(_) => "non_loopback_bind",
            Self::EmptyBearerToken => "empty_bearer_token",
            Self::InvalidAllowedOrigin => "invalid_allowed_origin",
            Self::Http(_) => "http_error",
            Self::Json(_) => "invalid_json",
            Self::Io(_) => "io_error",
        }
    }

    pub fn api_error(&self) -> ApiError {
        ApiError {
            code: self.code().to_owned(),
            message: self.to_string(),
        }
    }

    fn http_status(&self) -> u16 {
        match self {
            Self::Security(_) => 403,
            Self::PlanChanged => 409,
            Self::NoExecutableSteps => 422,
            Self::InvalidGraph(_) | Self::Planning(_) | Self::Inspection(_) | Self::Json(_) => 422,
            Self::Http(_) => 400,
            _ => 500,
        }
    }
}

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedConfigVersion(found) => write!(
                f,
                "unsupported bridge config schema_version '{found}', expected '{BRIDGE_CONFIG_SCHEMA_VERSION}'"
            ),
            Self::EmptyImplementationRef => {
                write!(f, "trusted implementation_ref must not be empty")
            }
            Self::InvalidImplementation(message) => {
                write!(f, "invalid trusted implementation: {message}")
            }
            Self::InvalidConnector(message) => {
                write!(f, "invalid trusted connector: {message}")
            }
            Self::InvalidGraph(message) => write!(f, "invalid Graph: {message}"),
            Self::Planning(message) => write!(f, "planning failed: {message}"),
            Self::NoExecutableSteps => write!(
                f,
                "Graph lowered to no executable steps; direct nested Composite expansion is not part of GUI-S5"
            ),
            Self::Inspection(message) => {
                write!(f, "execution access inspection failed: {message}")
            }
            Self::PlanChanged => write!(
                f,
                "host replanning produced a different ExecutionPlan; preview again before running"
            ),
            Self::Security(message) => write!(f, "guarded execution denied: {message}"),
            Self::NonLoopbackBind(ip) => {
                write!(f, "runtime bridge must bind a loopback address, got '{ip}'")
            }
            Self::EmptyBearerToken => write!(f, "runtime bridge bearer token must not be empty"),
            Self::InvalidAllowedOrigin => write!(
                f,
                "runtime bridge allowed Origin must be an explicit non-wildcard http(s) origin"
            ),
            Self::Http(message) => write!(f, "invalid HTTP request: {message}"),
            Self::Json(error) => write!(f, "invalid JSON: {error}"),
            Self::Io(error) => write!(f, "I/O error: {error}"),
        }
    }
}

impl Error for BridgeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for BridgeError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<std::io::Error> for BridgeError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

pub struct RuntimeBridgeService {
    implementations: ImplementationRegistry,
    routes: RouteRegistry,
}

impl RuntimeBridgeService {
    pub fn new(implementations: ImplementationRegistry, routes: RouteRegistry) -> Self {
        Self {
            implementations,
            routes,
        }
    }

    pub fn from_config(config: RuntimeBridgeConfig) -> Result<Self, BridgeError> {
        if config.schema_version != BRIDGE_CONFIG_SCHEMA_VERSION {
            return Err(BridgeError::UnsupportedConfigVersion(config.schema_version));
        }

        let mut implementations = ImplementationRegistry::new();
        for trusted in config.implementations {
            if trusted.implementation_ref.trim().is_empty() {
                return Err(BridgeError::EmptyImplementationRef);
            }
            if trusted.action.program.trim().is_empty() {
                return Err(BridgeError::InvalidImplementation(format!(
                    "implementation '{}' must declare a non-empty program",
                    trusted.implementation_ref
                )));
            }
            implementations
                .register(trusted.implementation_ref, trusted.action)
                .map_err(|error| BridgeError::InvalidImplementation(error.to_string()))?;
        }

        let mut routes = RouteRegistry::new();
        for connector in config.connectors {
            routes
                .register(connector)
                .map_err(|error| BridgeError::InvalidConnector(error.to_string()))?;
        }

        Ok(Self::new(implementations, routes))
    }

    pub fn plan(&self, graph: &Graph) -> Result<PlanResponse, BridgeError> {
        graph
            .validate()
            .map_err(|error| BridgeError::InvalidGraph(error.to_string()))?;

        let plan = Planner::lower(graph, &self.implementations, &self.routes)
            .map_err(|error| BridgeError::Planning(error.to_string()))?;
        if plan.steps.is_empty() {
            return Err(BridgeError::NoExecutableSteps);
        }

        let access_report = GuardedProcessRuntime::inspect(&plan, &self.implementations)
            .map_err(|error| BridgeError::Inspection(error.to_string()))?;

        Ok(PlanResponse {
            plan,
            access_report,
        })
    }

    pub fn run(&self, request: &RunRequest) -> Result<RunResponse, BridgeError> {
        let preview = self.plan(&request.graph)?;
        if preview.plan != request.expected_plan {
            return Err(BridgeError::PlanChanged);
        }

        let mut policy = ExecutionPolicy::new();
        let mut grants = BTreeSet::new();
        for implementation_ref in &request.allowed_implementation_refs {
            if grants.insert(implementation_ref.as_str()) {
                policy.allow(implementation_ref.clone());
            }
        }

        let access_report =
            GuardedProcessRuntime::preflight(&preview.plan, &self.implementations, &policy)
                .map_err(|error| BridgeError::Security(error.to_string()))?;

        let trace = GuardedProcessRuntime::execute(&preview.plan, &self.implementations, &policy)
            .map_err(|error| BridgeError::Security(error.to_string()))?;

        Ok(RunResponse {
            access_report,
            trace,
        })
    }

    pub fn health(&self) -> HealthResponse {
        HealthResponse {
            schema_version: BRIDGE_CONFIG_SCHEMA_VERSION.to_owned(),
            status: "ready".to_owned(),
        }
    }
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

#[derive(Debug)]
struct HttpResponse {
    status: u16,
    body: Vec<u8>,
    content_type: &'static str,
    cors_origin: Option<String>,
    extra_headers: Vec<(String, String)>,
}

impl HttpResponse {
    fn json<T: Serialize>(
        status: u16,
        value: &T,
        cors_origin: Option<String>,
    ) -> Result<Self, BridgeError> {
        Ok(Self {
            status,
            body: serde_json::to_vec(value)?,
            content_type: "application/json; charset=utf-8",
            cors_origin,
            extra_headers: Vec::new(),
        })
    }

    fn empty(status: u16, cors_origin: Option<String>) -> Self {
        Self {
            status,
            body: Vec::new(),
            content_type: "text/plain; charset=utf-8",
            cors_origin,
            extra_headers: Vec::new(),
        }
    }
}

pub struct RuntimeBridgeServer {
    listener: TcpListener,
    service: Arc<RuntimeBridgeService>,
    bearer_token: Arc<str>,
    allowed_origin: Arc<str>,
}

impl RuntimeBridgeServer {
    pub fn bind(
        service: RuntimeBridgeService,
        bind: SocketAddr,
        bearer_token: impl Into<String>,
        allowed_origin: impl Into<String>,
    ) -> Result<Self, BridgeError> {
        if !bind.ip().is_loopback() {
            return Err(BridgeError::NonLoopbackBind(bind.ip()));
        }

        let bearer_token = bearer_token.into();
        if bearer_token.trim().is_empty() {
            return Err(BridgeError::EmptyBearerToken);
        }

        let allowed_origin = allowed_origin.into();
        if !valid_allowed_origin(&allowed_origin) {
            return Err(BridgeError::InvalidAllowedOrigin);
        }

        let listener = TcpListener::bind(bind)?;
        Ok(Self {
            listener,
            service: Arc::new(service),
            bearer_token: Arc::from(bearer_token),
            allowed_origin: Arc::from(allowed_origin),
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, BridgeError> {
        Ok(self.listener.local_addr()?)
    }

    pub fn serve(self) -> Result<(), BridgeError> {
        for connection in self.listener.incoming() {
            let stream = connection?;
            let service = Arc::clone(&self.service);
            let token = Arc::clone(&self.bearer_token);
            let origin = Arc::clone(&self.allowed_origin);
            thread::spawn(move || {
                let _ = handle_connection(stream, &service, &token, &origin);
            });
        }
        Ok(())
    }

    pub fn serve_one(&self) -> Result<(), BridgeError> {
        let (stream, _) = self.listener.accept()?;
        handle_connection(
            stream,
            &self.service,
            &self.bearer_token,
            &self.allowed_origin,
        )
    }
}

fn valid_allowed_origin(origin: &str) -> bool {
    let trimmed = origin.trim();
    if trimmed != origin
        || trimmed.is_empty()
        || trimmed.contains(['\r', '\n', '*'])
    {
        return false;
    }

    let authority = trimmed
        .strip_prefix("http://")
        .or_else(|| trimmed.strip_prefix("https://"));
    let Some(authority) = authority else {
        return false;
    };

    !authority.is_empty()
        && !authority.contains(['/', '?', '#', '@', ' ', '\t'])
}

fn handle_connection(
    mut stream: TcpStream,
    service: &RuntimeBridgeService,
    bearer_token: &str,
    allowed_origin: &str,
) -> Result<(), BridgeError> {
    let timeout = Some(std::time::Duration::from_secs(5));
    stream.set_read_timeout(timeout)?;
    stream.set_write_timeout(timeout)?;

    let request = match read_http_request(&mut stream) {
        Ok(request) => request,
        Err(error) => {
            let response = HttpResponse::json(400, &error.api_error(), None)?;
            return write_http_response(&mut stream, response);
        }
    };

    let response = route_http_request(request, service, bearer_token, allowed_origin)
        .unwrap_or_else(|error| {
            HttpResponse::json(error.http_status(), &error.api_error(), None)
                .unwrap_or_else(|_| HttpResponse::empty(500, None))
        });

    write_http_response(&mut stream, response)
}

fn route_http_request(
    request: HttpRequest,
    service: &RuntimeBridgeService,
    bearer_token: &str,
    allowed_origin: &str,
) -> Result<HttpResponse, BridgeError> {
    let origin = request
        .headers
        .get("origin")
        .ok_or_else(|| BridgeError::Http("missing Origin header".to_owned()))?;
    if origin != allowed_origin {
        return Ok(HttpResponse::json(
            403,
            &ApiError {
                code: "origin_denied".to_owned(),
                message: "request Origin is not allowed".to_owned(),
            },
            None,
        )?);
    }

    if request.method == "OPTIONS" {
        let mut response = HttpResponse::empty(204, Some(allowed_origin.to_owned()));
        response.extra_headers.extend([
            (
                "Access-Control-Allow-Methods".to_owned(),
                "GET, POST, OPTIONS".to_owned(),
            ),
            (
                "Access-Control-Allow-Headers".to_owned(),
                "authorization, content-type".to_owned(),
            ),
            ("Access-Control-Max-Age".to_owned(), "600".to_owned()),
        ]);
        return Ok(response);
    }

    let expected_authorization = format!("Bearer {bearer_token}");
    if request.headers.get("authorization") != Some(&expected_authorization) {
        return Ok(HttpResponse::json(
            401,
            &ApiError {
                code: "unauthorized".to_owned(),
                message: "valid runtime bridge bearer token required".to_owned(),
            },
            Some(allowed_origin.to_owned()),
        )?);
    }

    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/v1/health") => {
            HttpResponse::json(200, &service.health(), Some(allowed_origin.to_owned()))
        }
        ("POST", "/v1/plan") => {
            let request: PlanRequest = serde_json::from_slice(&request.body)?;
            match service.plan(&request.graph) {
                Ok(response) => HttpResponse::json(200, &response, Some(allowed_origin.to_owned())),
                Err(error) => HttpResponse::json(
                    error.http_status(),
                    &error.api_error(),
                    Some(allowed_origin.to_owned()),
                ),
            }
        }
        ("POST", "/v1/run") => {
            let request: RunRequest = serde_json::from_slice(&request.body)?;
            match service.run(&request) {
                Ok(response) => HttpResponse::json(200, &response, Some(allowed_origin.to_owned())),
                Err(error) => HttpResponse::json(
                    error.http_status(),
                    &error.api_error(),
                    Some(allowed_origin.to_owned()),
                ),
            }
        }
        _ => HttpResponse::json(
            404,
            &ApiError {
                code: "not_found".to_owned(),
                message: "runtime bridge endpoint not found".to_owned(),
            },
            Some(allowed_origin.to_owned()),
        ),
    }
}

fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest, BridgeError> {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    if request_line.len() > MAX_HTTP_HEADER_BYTES {
        return Err(BridgeError::Http("request line too large".to_owned()));
    }

    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| BridgeError::Http("missing method".to_owned()))?
        .to_owned();
    let path = parts
        .next()
        .ok_or_else(|| BridgeError::Http("missing path".to_owned()))?
        .to_owned();
    let version = parts
        .next()
        .ok_or_else(|| BridgeError::Http("missing HTTP version".to_owned()))?;
    if version != "HTTP/1.1" {
        return Err(BridgeError::Http(
            "runtime bridge requires HTTP/1.1".to_owned(),
        ));
    }

    let mut headers = HashMap::new();
    let mut header_bytes = request_line.len();
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            return Err(BridgeError::Http(
                "unexpected EOF while reading headers".to_owned(),
            ));
        }
        header_bytes += read;
        if header_bytes > MAX_HTTP_HEADER_BYTES {
            return Err(BridgeError::Http("request headers too large".to_owned()));
        }
        if line == "\r\n" || line == "\n" {
            break;
        }

        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| BridgeError::Http("malformed header".to_owned()))?;
        let name = name.trim().to_ascii_lowercase();
        if headers
            .insert(name.clone(), value.trim().to_owned())
            .is_some()
        {
            return Err(BridgeError::Http(format!(
                "duplicate HTTP header '{name}'"
            )));
        }
    }

    let content_length = headers
        .get("content-length")
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|_| BridgeError::Http("invalid Content-Length".to_owned()))
        })
        .transpose()?
        .unwrap_or(0);
    if content_length > MAX_HTTP_BODY_BYTES {
        return Err(BridgeError::Http(format!(
            "request body exceeds {MAX_HTTP_BODY_BYTES} bytes"
        )));
    }

    let mut body = vec![0; content_length];
    reader.read_exact(&mut body)?;

    Ok(HttpRequest {
        method,
        path,
        headers,
        body,
    })
}

fn write_http_response(stream: &mut TcpStream, response: HttpResponse) -> Result<(), BridgeError> {
    let mut writer = BufWriter::new(stream);
    let reason = match response.status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        413 => "Payload Too Large",
        422 => "Unprocessable Entity",
        _ => "Internal Server Error",
    };

    write!(writer, "HTTP/1.1 {} {}\r\n", response.status, reason)?;
    write!(writer, "Content-Type: {}\r\n", response.content_type)?;
    write!(writer, "Content-Length: {}\r\n", response.body.len())?;
    write!(writer, "Connection: close\r\n")?;
    if let Some(origin) = response.cors_origin {
        write!(writer, "Access-Control-Allow-Origin: {origin}\r\n")?;
        write!(writer, "Vary: Origin\r\n")?;
    }
    for (name, value) in response.extra_headers {
        write!(writer, "{name}: {value}\r\n")?;
    }
    write!(writer, "\r\n")?;
    writer.write_all(&response.body)?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use algoram_core::{Block, Extensions};
    use algoram_runtime::{ExecutionSecurityError, TraceStatus};
    use std::env;

    fn executable_graph(implementation_ref: &str) -> Graph {
        let mut graph = Graph::new("graph:bridge-test");
        graph.blocks.push(Block {
            id: "block:bridge-test".to_owned(),
            label: "Bridge test".to_owned(),
            ports: Vec::new(),
            internal_graph_ref: None,
            implementation_ref: Some(implementation_ref.to_owned()),
            definition_ref: None,
            source_anchor: None,
            extensions: Extensions::new(),
            diagnostics: Vec::new(),
        });
        graph
    }

    fn test_service() -> RuntimeBridgeService {
        let mut implementations = ImplementationRegistry::new();
        implementations
            .register(
                "impl:bridge-test",
                ProcessAction::new(
                    env::current_exe().unwrap().display().to_string(),
                    ["--list"],
                ),
            )
            .unwrap();
        RuntimeBridgeService::new(implementations, RouteRegistry::new())
    }

    #[test]
    fn plan_inspects_trusted_action_and_run_requires_explicit_grant() {
        let service = test_service();
        let graph = executable_graph("impl:bridge-test");
        let preview = service.plan(&graph).unwrap();
        assert_eq!(preview.plan.steps.len(), 1);
        assert_eq!(preview.access_report.requirements.len(), 1);
        assert_eq!(
            preview.access_report.requirements[0].implementation_ref,
            "impl:bridge-test"
        );

        let denied = service
            .run(&RunRequest {
                graph: graph.clone(),
                expected_plan: preview.plan.clone(),
                allowed_implementation_refs: Vec::new(),
            })
            .unwrap_err();
        assert!(matches!(denied, BridgeError::Security(_)));

        let executed = service
            .run(&RunRequest {
                graph,
                expected_plan: preview.plan,
                allowed_implementation_refs: vec!["impl:bridge-test".to_owned()],
            })
            .unwrap();
        assert!(executed.trace.succeeded());
        assert_eq!(executed.trace.entries[0].status, TraceStatus::Succeeded);
    }

    #[test]
    fn host_replanning_rejects_tampered_expected_plan() {
        let service = test_service();
        let graph = executable_graph("impl:bridge-test");
        let preview = service.plan(&graph).unwrap();
        let mut tampered = preview.plan;
        tampered.steps[0].action.args.push("tampered".to_owned());

        let error = service
            .run(&RunRequest {
                graph,
                expected_plan: tampered,
                allowed_implementation_refs: vec!["impl:bridge-test".to_owned()],
            })
            .unwrap_err();
        assert!(matches!(error, BridgeError::PlanChanged));
    }

    #[test]
    fn no_executable_steps_is_not_success() {
        let service = test_service();
        let graph = Graph::new("graph:structural-only");
        assert!(matches!(
            service.plan(&graph),
            Err(BridgeError::NoExecutableSteps)
        ));
    }

    #[test]
    fn bridge_refuses_non_loopback_bind_and_weak_origin_configuration() {
        let service = test_service();
        let any = "0.0.0.0:0".parse().unwrap();
        assert!(matches!(
            RuntimeBridgeServer::bind(service, any, "token", "http://127.0.0.1:5173"),
            Err(BridgeError::NonLoopbackBind(_))
        ));

        assert!(!valid_allowed_origin("*"));
        assert!(!valid_allowed_origin(""));
        assert!(valid_allowed_origin("http://127.0.0.1:5173"));
        assert!(valid_allowed_origin("https://editor.example"));
    }

    #[test]
    fn config_builds_host_owned_registry() {
        let config = RuntimeBridgeConfig {
            schema_version: BRIDGE_CONFIG_SCHEMA_VERSION.to_owned(),
            implementations: vec![TrustedImplementationConfig {
                implementation_ref: "impl:bridge-test".to_owned(),
                action: ProcessAction::new(
                    env::current_exe().unwrap().display().to_string(),
                    ["--list"],
                ),
            }],
            connectors: Vec::new(),
        };
        let service = RuntimeBridgeService::from_config(config).unwrap();
        assert!(service.plan(&executable_graph("impl:bridge-test")).is_ok());
    }

    #[test]
    fn raw_guard_error_remains_phase7_authority() {
        let mut implementations = ImplementationRegistry::new();
        let action = ProcessAction::new(
            env::current_exe().unwrap().display().to_string(),
            ["--list"],
        );
        implementations
            .register("impl:bridge-test", action.clone())
            .unwrap();
        let graph = executable_graph("impl:bridge-test");
        let plan = Planner::lower(&graph, &implementations, &RouteRegistry::new()).unwrap();
        let denied =
            GuardedProcessRuntime::execute(&plan, &implementations, &ExecutionPolicy::new())
                .unwrap_err();
        assert!(matches!(
            denied,
            ExecutionSecurityError::DeniedImplementation { .. }
        ));
    }
}
