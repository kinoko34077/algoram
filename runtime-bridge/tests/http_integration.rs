use algoram_core::{Block, Extensions, Graph};
use algoram_interop::RouteRegistry;
use algoram_runtime::{ImplementationRegistry, ProcessAction};
use algoram_runtime_bridge::{
    PlanRequest, PlanResponse, RunRequest, RuntimeBridgeServer, RuntimeBridgeService,
};
use std::env;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;

const ORIGIN: &str = "http://127.0.0.1:5173";
const TOKEN: &str = "bridge-test-token";
const IMPLEMENTATION_REF: &str = "impl:http-bridge-test";

fn service() -> RuntimeBridgeService {
    let mut implementations = ImplementationRegistry::new();
    implementations
        .register(
            IMPLEMENTATION_REF,
            ProcessAction::new(
                env::current_exe().unwrap().display().to_string(),
                ["--list"],
            ),
        )
        .unwrap();
    RuntimeBridgeService::new(implementations, RouteRegistry::new())
}

fn graph() -> Graph {
    let mut graph = Graph::new("graph:http-bridge-test");
    graph.blocks.push(Block {
        id: "block:http-bridge-test".to_owned(),
        label: "HTTP bridge test".to_owned(),
        ports: Vec::new(),
        internal_graph_ref: None,
        implementation_ref: Some(IMPLEMENTATION_REF.to_owned()),
        definition_ref: None,
        source_anchor: None,
        extensions: Extensions::new(),
        diagnostics: Vec::new(),
    });
    graph
}

fn request(
    method: &str,
    path: &str,
    origin: &str,
    auth: Option<&str>,
    body: Option<&str>,
    extra_headers: &[(&str, &str)],
) -> String {
    let body = body.unwrap_or("");
    let mut request =
        format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: {origin}\r\n");
    if let Some(auth) = auth {
        request.push_str(&format!("Authorization: {auth}\r\n"));
    }
    for (name, value) in extra_headers {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    if !body.is_empty() {
        request.push_str("Content-Type: application/json\r\n");
    }
    request.push_str(&format!("Content-Length: {}\r\n", body.len()));
    request.push_str("Connection: close\r\n\r\n");
    request.push_str(body);
    request
}

fn exchange(service: RuntimeBridgeService, request: String) -> String {
    let server =
        RuntimeBridgeServer::bind(service, "127.0.0.1:0".parse().unwrap(), TOKEN, ORIGIN).unwrap();
    let addr = server.local_addr().unwrap();
    let server_thread = thread::spawn(move || server.serve_one());

    let mut stream = TcpStream::connect(addr).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    stream.flush().unwrap();

    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    server_thread.join().unwrap().unwrap();
    response
}

fn status(response: &str) -> u16 {
    response
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap()
}

fn body(response: &str) -> &str {
    response.split_once("\r\n\r\n").unwrap().1
}

fn header<'a>(response: &'a str, name: &str) -> Option<&'a str> {
    response
        .split_once("\r\n\r\n")
        .unwrap()
        .0
        .lines()
        .skip(1)
        .find_map(|line| {
            let (header_name, value) = line.split_once(':')?;
            header_name
                .eq_ignore_ascii_case(name)
                .then_some(value.trim())
        })
}

#[test]
fn browser_preflight_uses_exact_origin_without_bearer() {
    let response = exchange(
        service(),
        request(
            "OPTIONS",
            "/v1/run",
            ORIGIN,
            None,
            None,
            &[
                ("Access-Control-Request-Method", "POST"),
                (
                    "Access-Control-Request-Headers",
                    "authorization, content-type",
                ),
            ],
        ),
    );

    assert_eq!(status(&response), 204);
    assert_eq!(
        header(&response, "Access-Control-Allow-Origin"),
        Some(ORIGIN)
    );
    assert_eq!(
        header(&response, "Access-Control-Allow-Methods"),
        Some("GET, POST, OPTIONS")
    );
    assert_ne!(header(&response, "Access-Control-Allow-Origin"), Some("*"));
}

#[test]
fn wrong_origin_and_missing_bearer_are_rejected() {
    let wrong_origin = exchange(
        service(),
        request(
            "GET",
            "/v1/health",
            "http://127.0.0.1:5999",
            Some(&format!("Bearer {TOKEN}")),
            None,
            &[],
        ),
    );
    assert_eq!(status(&wrong_origin), 403);
    assert_eq!(header(&wrong_origin, "Access-Control-Allow-Origin"), None);

    let missing_bearer = exchange(
        service(),
        request("GET", "/v1/health", ORIGIN, None, None, &[]),
    );
    assert_eq!(status(&missing_bearer), 401);
    assert_eq!(
        header(&missing_bearer, "Access-Control-Allow-Origin"),
        Some(ORIGIN)
    );
}

#[test]
fn real_http_plan_default_deny_allow_and_plan_mismatch() {
    let graph = graph();
    let plan_body = serde_json::to_string(&PlanRequest {
        graph: graph.clone(),
    })
    .unwrap();
    let plan_response = exchange(
        service(),
        request(
            "POST",
            "/v1/plan",
            ORIGIN,
            Some(&format!("Bearer {TOKEN}")),
            Some(&plan_body),
            &[],
        ),
    );
    assert_eq!(status(&plan_response), 200);
    assert_eq!(
        header(&plan_response, "Access-Control-Allow-Origin"),
        Some(ORIGIN)
    );

    let preview: PlanResponse = serde_json::from_str(body(&plan_response)).unwrap();
    assert_eq!(preview.plan.steps.len(), 1);
    assert_eq!(preview.access_report.requirements.len(), 1);
    assert_eq!(
        preview.access_report.requirements[0].implementation_ref,
        IMPLEMENTATION_REF
    );

    let denied_body = serde_json::to_string(&RunRequest {
        graph: graph.clone(),
        expected_plan: preview.plan.clone(),
        allowed_implementation_refs: Vec::new(),
    })
    .unwrap();
    let denied_response = exchange(
        service(),
        request(
            "POST",
            "/v1/run",
            ORIGIN,
            Some(&format!("Bearer {TOKEN}")),
            Some(&denied_body),
            &[],
        ),
    );
    assert_eq!(status(&denied_response), 403);
    assert!(body(&denied_response).contains("execution_denied"));

    let allowed_body = serde_json::to_string(&RunRequest {
        graph: graph.clone(),
        expected_plan: preview.plan.clone(),
        allowed_implementation_refs: vec![IMPLEMENTATION_REF.to_owned()],
    })
    .unwrap();
    let allowed_response = exchange(
        service(),
        request(
            "POST",
            "/v1/run",
            ORIGIN,
            Some(&format!("Bearer {TOKEN}")),
            Some(&allowed_body),
            &[],
        ),
    );
    assert_eq!(status(&allowed_response), 200);
    assert!(body(&allowed_response).contains("\"status\":\"succeeded\""));

    let mut tampered = preview.plan;
    tampered.steps[0].action.args.push("--tampered".to_owned());
    let tampered_body = serde_json::to_string(&RunRequest {
        graph,
        expected_plan: tampered,
        allowed_implementation_refs: vec![IMPLEMENTATION_REF.to_owned()],
    })
    .unwrap();
    let tampered_response = exchange(
        service(),
        request(
            "POST",
            "/v1/run",
            ORIGIN,
            Some(&format!("Bearer {TOKEN}")),
            Some(&tampered_body),
            &[],
        ),
    );
    assert_eq!(status(&tampered_response), 409);
    assert!(body(&tampered_response).contains("plan_changed"));
}

#[test]
fn server_refuses_non_loopback_or_non_origin_configuration() {
    let non_loopback =
        RuntimeBridgeServer::bind(service(), "0.0.0.0:0".parse().unwrap(), TOKEN, ORIGIN);
    assert!(non_loopback.is_err());

    for invalid_origin in [
        "*",
        "http://127.0.0.1:5173/path",
        "http://127.0.0.1:5173?query",
        "http://user@127.0.0.1:5173",
        " http://127.0.0.1:5173",
    ] {
        let result = RuntimeBridgeServer::bind(
            service(),
            "127.0.0.1:0".parse().unwrap(),
            TOKEN,
            invalid_origin,
        );
        assert!(
            result.is_err(),
            "origin should be rejected: {invalid_origin}"
        );
    }
}
