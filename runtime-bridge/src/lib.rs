use algoram_catalog::CapabilityCatalog;
use algoram_core::Graph;
use algoram_interop::{Connector, RouteRegistry};
use algoram_resilience::{
    AlternativeRouteCandidate, ImplementationRecoveryCandidates, ResilienceAnalyzer,
    ResilienceReport, RuntimeRecoveryCandidate,
};
use algoram_runtime::{
    ExecutionAccessReport, ExecutionPlan, ExecutionPolicy, ExecutionTrace, GuardedProcessRuntime,
    ImplementationRegistry, PlacedExecutionPlan, Planner, ProcessAction, RouteOverride,
    RuntimeEndpoint, RuntimeLocationClass, StepPlacement,
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
pub struct TrustedImplementationChoiceConfig {
    pub logical_ref: String,
    pub candidates: Vec<String>,
    pub default_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeBridgeConfig {
    pub schema_version: String,
    #[serde(default)]
    pub implementations: Vec<TrustedImplementationConfig>,
    #[serde(default)]
    pub implementation_choices: Vec<TrustedImplementationChoiceConfig>,
    #[serde(default)]
    pub connectors: Vec<Connector>,
    #[serde(default)]
    pub runtime_endpoints: Vec<RuntimeEndpoint>,
    #[serde(default)]
    pub local_runtime_ref: Option<String>,
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
pub struct ImplementationOverride {
    pub logical_ref: String,
    pub implementation_ref: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoverySelections {
    #[serde(default)]
    pub route_overrides: Vec<RouteOverride>,
    #[serde(default)]
    pub implementation_overrides: Vec<ImplementationOverride>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecoveryPlanRequest {
    pub graph: Graph,
    #[serde(default)]
    pub selections: RecoverySelections,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecoveryRunRequest {
    pub graph: Graph,
    pub expected_plan: ExecutionPlan,
    #[serde(default)]
    pub allowed_implementation_refs: Vec<String>,
    #[serde(default)]
    pub selections: RecoverySelections,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecoveryOptionsRequest {
    pub graph: Graph,
    pub expected_plan: ExecutionPlan,
    pub trace: ExecutionTrace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepImplementationRecovery {
    pub step_id: String,
    pub selected_implementation_ref: String,
    pub candidates: ImplementationRecoveryCandidates,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeRecoveryOption {
    pub candidate: RuntimeRecoveryCandidate,
    pub placement_validated: bool,
    pub executable_by_bridge: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepRuntimeRecovery {
    pub step_id: String,
    pub current_runtime_ref: String,
    pub candidates: Vec<RuntimeRecoveryOption>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecoveryOptionsResponse {
    pub report: ResilienceReport,
    pub route_candidates: Vec<AlternativeRouteCandidate>,
    pub implementation_candidates: Vec<StepImplementationRecovery>,
    pub runtime_candidates: Vec<StepRuntimeRecovery>,
    pub catalog_connected: bool,
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
    InvalidRecoveryConfig(String),
    InvalidGraph(String),
    Planning(String),
    Recovery(String),
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
            Self::InvalidRecoveryConfig(_) => "invalid_recovery_config",
            Self::InvalidGraph(_) => "invalid_graph",
            Self::Planning(_) => "planning_failed",
            Self::Recovery(_) => "recovery_failed",
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
            Self::InvalidGraph(_)
            | Self::Planning(_)
            | Self::Recovery(_)
            | Self::Inspection(_)
            | Self::Json(_) => 422,
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
            Self::InvalidRecoveryConfig(message) => {
                write!(f, "invalid recovery bridge config: {message}")
            }
            Self::InvalidGraph(message) => write!(f, "invalid Graph: {message}"),
            Self::Planning(message) => write!(f, "planning failed: {message}"),
            Self::Recovery(message) => write!(f, "recovery analysis failed: {message}"),
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
    runtime_endpoints: Vec<RuntimeEndpoint>,
    local_runtime_ref: String,
}

impl RuntimeBridgeService {
    pub fn new(implementations: ImplementationRegistry, routes: RouteRegistry) -> Self {
        Self {
            implementations,
            routes,
            runtime_endpoints: Vec::new(),
            local_runtime_ref: "runtime:local-bridge".to_owned(),
        }
    }

    pub fn new_with_recovery(
        implementations: ImplementationRegistry,
        routes: RouteRegistry,
        runtime_endpoints: Vec<RuntimeEndpoint>,
        local_runtime_ref: impl Into<String>,
    ) -> Result<Self, BridgeError> {
        let local_runtime_ref = local_runtime_ref.into();
        if local_runtime_ref.trim().is_empty() {
            return Err(BridgeError::InvalidRecoveryConfig(
                "local_runtime_ref must not be empty".to_owned(),
            ));
        }

        let mut seen = BTreeSet::new();
        for endpoint in &runtime_endpoints {
            if endpoint.runtime_ref.trim().is_empty() {
                return Err(BridgeError::InvalidRecoveryConfig(
                    "runtime endpoint ref must not be empty".to_owned(),
                ));
            }
            if !seen.insert(endpoint.runtime_ref.as_str()) {
                return Err(BridgeError::InvalidRecoveryConfig(format!(
                    "duplicate runtime endpoint '{}'",
                    endpoint.runtime_ref
                )));
            }
        }

        Ok(Self {
            implementations,
            routes,
            runtime_endpoints,
            local_runtime_ref,
        })
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

        for choice in config.implementation_choices {
            implementations
                .register_choice(choice.logical_ref, choice.candidates, choice.default_ref)
                .map_err(|error| BridgeError::InvalidImplementation(error.to_string()))?;
        }

        let mut routes = RouteRegistry::new();
        for connector in config.connectors {
            routes
                .register(connector)
                .map_err(|error| BridgeError::InvalidConnector(error.to_string()))?;
        }

        Self::new_with_recovery(
            implementations,
            routes,
            config.runtime_endpoints,
            config
                .local_runtime_ref
                .unwrap_or_else(|| "runtime:local-bridge".to_owned()),
        )
    }

    fn selected_implementations(
        &self,
        graph: &Graph,
        overrides: &[ImplementationOverride],
    ) -> Result<ImplementationRegistry, BridgeError> {
        let mut selected = self.implementations.clone();
        let mut seen = BTreeSet::new();

        for implementation_override in overrides {
            if implementation_override.logical_ref.trim().is_empty()
                || implementation_override.implementation_ref.trim().is_empty()
            {
                return Err(BridgeError::Planning(
                    "implementation recovery override refs must not be empty".to_owned(),
                ));
            }
            if !seen.insert(implementation_override.logical_ref.as_str()) {
                return Err(BridgeError::Planning(format!(
                    "implementation recovery override for '{}' is declared more than once",
                    implementation_override.logical_ref
                )));
            }
            if !graph.blocks.iter().any(|block| {
                block.implementation_ref.as_deref()
                    == Some(implementation_override.logical_ref.as_str())
            }) {
                return Err(BridgeError::Planning(format!(
                    "implementation recovery override '{}' is not referenced by this Graph",
                    implementation_override.logical_ref
                )));
            }
            selected = selected
                .with_choice_default(
                    &implementation_override.logical_ref,
                    &implementation_override.implementation_ref,
                )
                .map_err(|error| BridgeError::Planning(error.to_string()))?;
        }

        Ok(selected)
    }

    fn plan_selected(
        &self,
        graph: &Graph,
        selections: &RecoverySelections,
    ) -> Result<(PlanResponse, ImplementationRegistry), BridgeError> {
        graph
            .validate()
            .map_err(|error| BridgeError::InvalidGraph(error.to_string()))?;

        let implementations =
            self.selected_implementations(graph, &selections.implementation_overrides)?;
        let plan = if selections.route_overrides.is_empty() {
            Planner::lower(graph, &implementations, &self.routes)
        } else {
            Planner::lower_with_route_overrides(
                graph,
                &implementations,
                &self.routes,
                &selections.route_overrides,
            )
        }
        .map_err(|error| BridgeError::Planning(error.to_string()))?;

        if plan.steps.is_empty() {
            return Err(BridgeError::NoExecutableSteps);
        }

        let access_report = GuardedProcessRuntime::inspect(&plan, &implementations)
            .map_err(|error| BridgeError::Inspection(error.to_string()))?;

        Ok((
            PlanResponse {
                plan,
                access_report,
            },
            implementations,
        ))
    }

    fn run_selected(
        &self,
        graph: &Graph,
        expected_plan: &ExecutionPlan,
        allowed_implementation_refs: &[String],
        selections: &RecoverySelections,
    ) -> Result<RunResponse, BridgeError> {
        let (preview, implementations) = self.plan_selected(graph, selections)?;
        if &preview.plan != expected_plan {
            return Err(BridgeError::PlanChanged);
        }

        let mut policy = ExecutionPolicy::new();
        let mut grants = BTreeSet::new();
        for implementation_ref in allowed_implementation_refs {
            if grants.insert(implementation_ref.as_str()) {
                policy.allow(implementation_ref.clone());
            }
        }

        let access_report =
            GuardedProcessRuntime::preflight(&preview.plan, &implementations, &policy)
                .map_err(|error| BridgeError::Security(error.to_string()))?;

        let trace = GuardedProcessRuntime::execute(&preview.plan, &implementations, &policy)
            .map_err(|error| BridgeError::Security(error.to_string()))?;

        Ok(RunResponse {
            access_report,
            trace,
        })
    }

    pub fn plan(&self, graph: &Graph) -> Result<PlanResponse, BridgeError> {
        self.plan_selected(graph, &RecoverySelections::default())
            .map(|(preview, _)| preview)
    }

    pub fn run(&self, request: &RunRequest) -> Result<RunResponse, BridgeError> {
        self.run_selected(
            &request.graph,
            &request.expected_plan,
            &request.allowed_implementation_refs,
            &RecoverySelections::default(),
        )
    }

    pub fn recovery_plan(
        &self,
        request: &RecoveryPlanRequest,
    ) -> Result<PlanResponse, BridgeError> {
        self.plan_selected(&request.graph, &request.selections)
            .map(|(preview, _)| preview)
    }

    pub fn recovery_run(&self, request: &RecoveryRunRequest) -> Result<RunResponse, BridgeError> {
        self.run_selected(
            &request.graph,
            &request.expected_plan,
            &request.allowed_implementation_refs,
            &request.selections,
        )
    }

    fn recovery_runtime_endpoints(&self, plan: &ExecutionPlan) -> Vec<RuntimeEndpoint> {
        let mut endpoints = self
            .runtime_endpoints
            .iter()
            .filter(|endpoint| endpoint.runtime_ref != self.local_runtime_ref)
            .cloned()
            .collect::<Vec<_>>();

        let local_implementations = plan
            .steps
            .iter()
            .map(|step| step.implementation_ref.clone())
            .collect::<BTreeSet<_>>();

        endpoints.push(RuntimeEndpoint::new(
            self.local_runtime_ref.clone(),
            RuntimeLocationClass::LocalProcess,
            local_implementations,
        ));
        endpoints.sort_by(|left, right| left.runtime_ref.cmp(&right.runtime_ref));
        endpoints
    }

    pub fn recovery_options(
        &self,
        request: &RecoveryOptionsRequest,
    ) -> Result<RecoveryOptionsResponse, BridgeError> {
        let report = ResilienceAnalyzer::analyze_execution(
            &request.graph,
            &request.expected_plan,
            &request.trace,
            &self.routes,
        )
        .map_err(|error| BridgeError::Recovery(error.to_string()))?;

        let route_candidates = ResilienceAnalyzer::alternative_route_candidates(
            &request.graph,
            &request.expected_plan,
            &self.routes,
        )
        .map_err(|error| BridgeError::Recovery(error.to_string()))?;

        let empty_catalog = CapabilityCatalog::new(Vec::new());
        let mut implementation_candidates = Vec::new();
        let mut seen_implementation_steps = BTreeSet::new();

        for failure in &report.failures {
            let Some(step) = request
                .expected_plan
                .steps
                .iter()
                .find(|step| step.id == failure.entry.step_id)
            else {
                continue;
            };

            for block_id in &step.origin_block_ids {
                let Some(block) = request
                    .graph
                    .blocks
                    .iter()
                    .find(|block| block.id == *block_id)
                else {
                    continue;
                };
                let Some(logical_ref) = block.implementation_ref.as_deref() else {
                    continue;
                };
                if self.implementations.choice(logical_ref).is_none()
                    || !seen_implementation_steps.insert((step.id.clone(), logical_ref.to_owned()))
                {
                    continue;
                }

                let candidates = ResilienceAnalyzer::implementation_recovery_candidates(
                    logical_ref,
                    &self.implementations,
                    &empty_catalog,
                )
                .map_err(|error| BridgeError::Recovery(error.to_string()))?;

                implementation_candidates.push(StepImplementationRecovery {
                    step_id: step.id.clone(),
                    selected_implementation_ref: step.implementation_ref.clone(),
                    candidates,
                });
            }
        }

        let endpoints = self.recovery_runtime_endpoints(&request.expected_plan);
        let mut runtime_candidates = Vec::new();
        for failure in &report.failures {
            let Some(step) = request
                .expected_plan
                .steps
                .iter()
                .find(|step| step.id == failure.entry.step_id)
            else {
                continue;
            };

            let candidates = ResilienceAnalyzer::runtime_recovery_candidates(
                step,
                &self.local_runtime_ref,
                &endpoints,
            )
            .into_iter()
            .map(|candidate| {
                let placements = request.expected_plan.steps.iter().map(|plan_step| {
                    StepPlacement::new(
                        plan_step.id.clone(),
                        if plan_step.id == step.id {
                            candidate.runtime_ref.clone()
                        } else {
                            self.local_runtime_ref.clone()
                        },
                    )
                });
                let placed = PlacedExecutionPlan::new(request.expected_plan.clone(), placements);
                let placement_validated = placed.validate(&endpoints).is_ok();
                let executable_by_bridge = placement_validated
                    && candidate.runtime_ref == self.local_runtime_ref
                    && candidate.class == RuntimeLocationClass::LocalProcess;

                RuntimeRecoveryOption {
                    candidate,
                    placement_validated,
                    executable_by_bridge,
                }
            })
            .collect::<Vec<_>>();

            runtime_candidates.push(StepRuntimeRecovery {
                step_id: step.id.clone(),
                current_runtime_ref: self.local_runtime_ref.clone(),
                candidates,
            });
        }

        Ok(RecoveryOptionsResponse {
            report,
            route_candidates,
            implementation_candidates,
            runtime_candidates,
            catalog_connected: false,
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
    if trimmed != origin || trimmed.is_empty() || trimmed.contains(['\r', '\n', '*']) {
        return false;
    }

    let authority = trimmed
        .strip_prefix("http://")
        .or_else(|| trimmed.strip_prefix("https://"));
    let Some(authority) = authority else {
        return false;
    };

    !authority.is_empty() && !authority.contains(['/', '?', '#', '@', ' ', '\t'])
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
        return HttpResponse::json(
            403,
            &ApiError {
                code: "origin_denied".to_owned(),
                message: "request Origin is not allowed".to_owned(),
            },
            None,
        );
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
        return HttpResponse::json(
            401,
            &ApiError {
                code: "unauthorized".to_owned(),
                message: "valid runtime bridge bearer token required".to_owned(),
            },
            Some(allowed_origin.to_owned()),
        );
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
        ("POST", "/v1/recovery/options") => {
            let request: RecoveryOptionsRequest = serde_json::from_slice(&request.body)?;
            match service.recovery_options(&request) {
                Ok(response) => HttpResponse::json(200, &response, Some(allowed_origin.to_owned())),
                Err(error) => HttpResponse::json(
                    error.http_status(),
                    &error.api_error(),
                    Some(allowed_origin.to_owned()),
                ),
            }
        }
        ("POST", "/v1/recovery/plan") => {
            let request: RecoveryPlanRequest = serde_json::from_slice(&request.body)?;
            match service.recovery_plan(&request) {
                Ok(response) => HttpResponse::json(200, &response, Some(allowed_origin.to_owned())),
                Err(error) => HttpResponse::json(
                    error.http_status(),
                    &error.api_error(),
                    Some(allowed_origin.to_owned()),
                ),
            }
        }
        ("POST", "/v1/recovery/run") => {
            let request: RecoveryRunRequest = serde_json::from_slice(&request.body)?;
            match service.recovery_run(&request) {
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
            return Err(BridgeError::Http(format!("duplicate HTTP header '{name}'")));
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
    use algoram_core::{Block, Connection, Extensions, Port, PortChannel, PortDirection, PortRef};
    use algoram_interop::{Connector, ContractId};
    use algoram_runtime::{ExecutionSecurityError, TraceEntry, TraceStatus};
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
            implementation_choices: Vec::new(),
            connectors: Vec::new(),
            runtime_endpoints: Vec::new(),
            local_runtime_ref: None,
        };
        let service = RuntimeBridgeService::from_config(config).unwrap();
        assert!(service.plan(&executable_graph("impl:bridge-test")).is_ok());
    }

    fn provider_service() -> RuntimeBridgeService {
        let mut implementations = ImplementationRegistry::new();
        implementations
            .register(
                "impl:provider-a",
                ProcessAction::new(
                    "algoram-recovery-test-must-not-execute",
                    std::iter::empty::<&str>(),
                ),
            )
            .unwrap();
        implementations
            .register(
                "impl:provider-b",
                ProcessAction::new(
                    "algoram-recovery-test-must-not-execute",
                    std::iter::empty::<&str>(),
                ),
            )
            .unwrap();
        implementations
            .register_choice(
                "logical:provider",
                ["impl:provider-a", "impl:provider-b"],
                "impl:provider-a",
            )
            .unwrap();

        RuntimeBridgeService::new_with_recovery(
            implementations,
            RouteRegistry::new(),
            vec![RuntimeEndpoint::new(
                "runtime:agent-alt",
                RuntimeLocationClass::RuntimeAgent,
                ["impl:provider-a", "impl:provider-b"],
            )],
            "runtime:local-bridge",
        )
        .unwrap()
    }

    fn failed_trace(plan: &ExecutionPlan) -> ExecutionTrace {
        let step = &plan.steps[0];
        ExecutionTrace {
            reference_graph_id: plan.reference_graph_id.clone(),
            entries: vec![TraceEntry {
                step_id: step.id.clone(),
                implementation_ref: step.implementation_ref.clone(),
                status: TraceStatus::Failed,
                origin_block_ids: step.origin_block_ids.clone(),
                source_anchors: step.source_anchors.clone(),
                route_connector_ids: step.route_connector_ids.clone(),
                exit_code: Some(23),
                stdout: String::new(),
                stderr: "provider unavailable".to_owned(),
            }],
        }
    }

    #[test]
    fn recovery_discovery_and_provider_replan_are_explicit_and_non_mutating() {
        let service = provider_service();
        let graph = executable_graph("logical:provider");
        let graph_before = graph.clone();
        let preview = service.plan(&graph).unwrap();
        assert_eq!(preview.plan.steps[0].implementation_ref, "impl:provider-a");

        let options = service
            .recovery_options(&RecoveryOptionsRequest {
                graph: graph.clone(),
                expected_plan: preview.plan.clone(),
                trace: failed_trace(&preview.plan),
            })
            .unwrap();

        assert_eq!(
            options.report.impact.failed_block_ids,
            vec!["block:bridge-test"]
        );
        assert_eq!(options.implementation_candidates.len(), 1);
        assert_eq!(
            options.implementation_candidates[0]
                .candidates
                .trusted_local
                .iter()
                .map(|candidate| (candidate.implementation_ref.as_str(), candidate.is_default))
                .collect::<Vec<_>>(),
            vec![("impl:provider-a", true), ("impl:provider-b", false)]
        );
        assert!(!options.catalog_connected);
        assert_eq!(options.runtime_candidates.len(), 1);
        assert!(options.runtime_candidates[0]
            .candidates
            .iter()
            .any(|candidate| {
                candidate.candidate.runtime_ref == "runtime:agent-alt"
                    && candidate.placement_validated
                    && !candidate.executable_by_bridge
            }));
        assert!(options.runtime_candidates[0]
            .candidates
            .iter()
            .any(|candidate| {
                candidate.candidate.runtime_ref == "runtime:local-bridge"
                    && candidate.placement_validated
                    && candidate.executable_by_bridge
            }));
        assert_eq!(graph, graph_before);

        let selections = RecoverySelections {
            route_overrides: Vec::new(),
            implementation_overrides: vec![ImplementationOverride {
                logical_ref: "logical:provider".to_owned(),
                implementation_ref: "impl:provider-b".to_owned(),
            }],
        };
        let recovered = service
            .recovery_plan(&RecoveryPlanRequest {
                graph: graph.clone(),
                selections,
            })
            .unwrap();
        assert_eq!(
            recovered.plan.steps[0].implementation_ref,
            "impl:provider-b"
        );

        let default_again = service.plan(&graph).unwrap();
        assert_eq!(
            default_again.plan.steps[0].implementation_ref,
            "impl:provider-a"
        );
        assert_eq!(graph, graph_before);
    }

    #[test]
    fn recovery_route_override_is_explicit_and_does_not_change_default_route() {
        let mut graph = Graph::new("graph:bridge-route-recovery");
        graph.blocks.push(Block {
            id: "block:route-source".to_owned(),
            label: "source".to_owned(),
            ports: vec![Port {
                id: "out".to_owned(),
                direction: PortDirection::Out,
                channel: PortChannel::Data,
                contract: Some(serde_json::Value::String("contract:a".to_owned())),
                extensions: Extensions::new(),
            }],
            internal_graph_ref: None,
            implementation_ref: None,
            definition_ref: None,
            source_anchor: None,
            extensions: Extensions::new(),
            diagnostics: Vec::new(),
        });
        graph.blocks.push(Block {
            id: "block:route-target".to_owned(),
            label: "target".to_owned(),
            ports: vec![Port {
                id: "in".to_owned(),
                direction: PortDirection::In,
                channel: PortChannel::Data,
                contract: Some(serde_json::Value::String("contract:c".to_owned())),
                extensions: Extensions::new(),
            }],
            internal_graph_ref: None,
            implementation_ref: Some("impl:route-target".to_owned()),
            definition_ref: None,
            source_anchor: None,
            extensions: Extensions::new(),
            diagnostics: Vec::new(),
        });
        graph.connections.push(Connection {
            id: "connection:route".to_owned(),
            source: PortRef {
                block_id: "block:route-source".to_owned(),
                port_id: "out".to_owned(),
            },
            target: PortRef {
                block_id: "block:route-target".to_owned(),
                port_id: "in".to_owned(),
            },
            extensions: Extensions::new(),
        });

        let mut implementations = ImplementationRegistry::new();
        implementations
            .register(
                "impl:route-target",
                ProcessAction::new(
                    "algoram-route-recovery-test-must-not-execute",
                    std::iter::empty::<&str>(),
                ),
            )
            .unwrap();

        let mut routes = RouteRegistry::new();
        routes
            .register(Connector::new(
                "connector:primary",
                ContractId::from("contract:a"),
                ContractId::from("contract:c"),
                "impl:connector-primary",
            ))
            .unwrap();
        routes
            .register(Connector::new(
                "connector:alt-a",
                ContractId::from("contract:a"),
                ContractId::from("contract:b"),
                "impl:connector-alt-a",
            ))
            .unwrap();
        routes
            .register(Connector::new(
                "connector:alt-b",
                ContractId::from("contract:b"),
                ContractId::from("contract:c"),
                "impl:connector-alt-b",
            ))
            .unwrap();

        let service = RuntimeBridgeService::new(implementations, routes);
        let default_plan = service.plan(&graph).unwrap();
        assert_eq!(
            default_plan.plan.steps[0].route_connector_ids,
            vec!["connector:primary"]
        );

        let recovered = service
            .recovery_plan(&RecoveryPlanRequest {
                graph: graph.clone(),
                selections: RecoverySelections {
                    route_overrides: vec![RouteOverride {
                        connection_id: "connection:route".to_owned(),
                        connector_ids: vec![
                            "connector:alt-a".to_owned(),
                            "connector:alt-b".to_owned(),
                        ],
                    }],
                    implementation_overrides: Vec::new(),
                },
            })
            .unwrap();
        assert_eq!(
            recovered.plan.steps[0].route_connector_ids,
            vec!["connector:alt-a", "connector:alt-b"]
        );

        let default_again = service.plan(&graph).unwrap();
        assert_eq!(
            default_again.plan.steps[0].route_connector_ids,
            vec!["connector:primary"]
        );
    }

    #[test]
    fn recovery_rejects_forged_trace_and_untrusted_provider_selection() {
        let service = provider_service();
        let graph = executable_graph("logical:provider");
        let preview = service.plan(&graph).unwrap();
        let mut trace = failed_trace(&preview.plan);
        trace.entries[0].implementation_ref = "impl:forged".to_owned();

        assert!(matches!(
            service.recovery_options(&RecoveryOptionsRequest {
                graph: graph.clone(),
                expected_plan: preview.plan.clone(),
                trace,
            }),
            Err(BridgeError::Recovery(_))
        ));

        assert!(matches!(
            service.recovery_plan(&RecoveryPlanRequest {
                graph,
                selections: RecoverySelections {
                    route_overrides: Vec::new(),
                    implementation_overrides: vec![ImplementationOverride {
                        logical_ref: "logical:provider".to_owned(),
                        implementation_ref: "impl:not-trusted".to_owned(),
                    }],
                },
            }),
            Err(BridgeError::Planning(_))
        ));
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
