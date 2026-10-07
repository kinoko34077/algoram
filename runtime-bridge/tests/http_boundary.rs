use algoram_core::{Block, Extensions, Graph};
use algoram_interop::RouteRegistry;
use algoram_runtime::{ImplementationRegistry, ProcessAction};
use algoram_runtime_bridge::{
    PlanRequest, PlanResponse, RunRequest, RunResponse, RuntimeBridgeServer, RuntimeBridgeService,
};
use std::env;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::thread;

const ORIGIN: &str = "http://127.0.0.1:5173";
const TOKEN: &str = "bridge-http-test-token";

fn executable_graph() -> Graph {
    let mut graph = Graph::new("graph:http-bridge-test");
    graph.blocks.push(Block {
        id: "block:http-bridge-test".to_owned(),
        label: "HTTP bridge test".to_owned(),
        ports: Vec::new(),
        internal_graph_ref: None,
        implementation_ref: Some("impl:http-bridge-test".to_owned()),
        definition_ref: None,
        source_anchor: None,
        extensions: Extensions::new(),
        diagnostics: Vec::new(),
    });
    graph
}

fn service() -> RuntimeBridgeService {
    let mut implementations = ImplementationRegistry::new();
    implementations
        .register(
            "impl:http-bridge-test",
            ProcessAction::new(env::current_exe().unwrap().display().to_string(), ["--list"]),
        )
        .unwrap();
    RuntimeBridgeService::new(implementations, RouteRegistry::new())
}

fn spawn_server(request_count: usize) -> (SocketAddr, thread::JoinHandle<()>) {
    let server = RuntimeBridgeServer::bind(
        service(),
        "127.0.0.1:0".parse().unwrap(),
        TOKEN,
        ORIGIN,
    )
    .unwrap();
    let addr = server.local_addr().unwrap();
    let handle = thread::spawn(move || {
        for _ in 0..request_count {
            server.serve_one().unwrap();
        }
    });
    (addr, handle)
}

fn send(addr: SocketAddr, request: &str) -> String {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    stream.flush().unwrap();

    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

fn response_body(response: &str) -> &str {
    response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or("")
}

fn post(addr: SocketAddr, path: &str, origin: &str, token: &str, body: &str) -> String {
    send(
        addr,
        &format!(
            "POST {path} HTTP/1.1\r\nHost: {addr}\r\nOrigin: {origin}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ),
    )
}

#[test]
fn loopback_http_boundary_enforces_origin_auth_and_guarded_run() {
    let (addr, server_thread) = spawn_server(7);

    let preflight = send(
        addr,
        &format!(
            "OPTIONS /v1/plan HTTP/1.1\r\nHost: {addr}\r\nOrigin: {ORIGIN}\r\nAccess-Control-Request-Method: POST\r\nAccess-Control-Request-Headers: authorization, content-type\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        ),
    );
    assert!(preflight.starts_with("HTTP/1.1 204"));
    assert!(preflight.contains(&format!("Access-Control-Allow-Origin: {ORIGIN}\r\n")));
    assert!(preflight.contains("Access-Control-Allow-Headers: authorization, content-type"));

    let graph = executable_graph();
    let plan_body = serde_json::to_string(&PlanRequest {
        graph: graph.clone(),
    })
    .unwrap();

    let wrong_origin = post(
        addr,
        "/v1/plan",
        "https://evil.example",
        TOKEN,
        &plan_body,
    );
    assert!(wrong_origin.starts_with("HTTP/1.1 403"));
    assert!(!wrong_origin.contains("Access-Control-Allow-Origin: *"));
    assert!(!wrong_origin.contains("Access-Control-Allow-Origin: https://evil.example"));

    let wrong_token = post(addr, "/v1/plan", ORIGIN, "wrong-token", &plan_body);
    assert!(wrong_token.starts_with("HTTP/1.1 401"));
    assert!(wrong_token.contains(&format!("Access-Control-Allow-Origin: {ORIGIN}\r\n")));

    let planned = post(addr, "/v1/plan", ORIGIN, TOKEN, &plan_body);
    assert!(planned.starts_with("HTTP/1.1 200"));
    let preview: PlanResponse = serde_json::from_str(response_body(&planned)).unwrap();
    assert_eq!(preview.plan.steps.len(), 1);
    assert_eq!(preview.access_report.requirements.len(), 1);
    assert_eq!(
        preview.access_report.requirements[0].implementation_ref,
        "impl:http-bridge-test"
    );

    let denied_body = serde_json::to_string(&RunRequest {
        graph: graph.clone(),
        expected_plan: preview.plan.clone(),
        allowed_implementation_refs: Vec::new(),
    })
    .unwrap();
    let denied = post(addr, "/v1/run", ORIGIN, TOKEN, &denied_body);
    assert!(denied.starts_with("HTTP/1.1 403"));
    assert!(response_body(&denied).contains("execution_denied"));

    let mut tampered_plan = preview.plan.clone();
    tampered_plan.steps[0].action.args.push("tampered".to_owned());
    let tampered_body = serde_json::to_string(&RunRequest {
        graph: graph.clone(),
        expected_plan: tampered_plan,
        allowed_implementation_refs: vec!["impl:http-bridge-test".to_owned()],
    })
    .unwrap();
    let tampered = post(addr, "/v1/run", ORIGIN, TOKEN, &tampered_body);
    assert!(tampered.starts_with("HTTP/1.1 409"));
    assert!(response_body(&tampered).contains("plan_changed"));

    let allowed_body = serde_json::to_string(&RunRequest {
        graph,
        expected_plan: preview.plan,
        allowed_implementation_refs: vec!["impl:http-bridge-test".to_owned()],
    })
    .unwrap();
    let allowed = post(addr, "/v1/run", ORIGIN, TOKEN, &allowed_body);
    assert!(allowed.starts_with("HTTP/1.1 200"));
    let executed: RunResponse = serde_json::from_str(response_body(&allowed)).unwrap();
    assert!(executed.trace.entries.iter().all(|entry| {
        entry.implementation_ref == "impl:http-bridge-test"
            && format!("{:?}", entry.status) == "Succeeded"
    }));

    server_thread.join().unwrap();
}
