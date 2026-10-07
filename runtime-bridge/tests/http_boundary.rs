use algoram_core::{Block, Extensions, Graph};
use algoram_interop::RouteRegistry;
use algoram_runtime::{
    ExecutionTrace, ImplementationRegistry, ProcessAction, RuntimeEndpoint, RuntimeLocationClass,
    TraceEntry, TraceStatus,
};
use algoram_runtime_bridge::{
    ImplementationOverride, PlanRequest, PlanResponse, RecoveryOptionsRequest,
    RecoveryOptionsResponse, RecoveryPlanRequest, RecoveryRunRequest, RecoverySelections,
    RunRequest, RunResponse, RuntimeBridgeServer, RuntimeBridgeService,
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
            ProcessAction::new(
                env::current_exe().unwrap().display().to_string(),
                ["--list"],
            ),
        )
        .unwrap();
    RuntimeBridgeService::new(implementations, RouteRegistry::new())
}

fn spawn_server_with(
    service: RuntimeBridgeService,
    request_count: usize,
) -> (SocketAddr, thread::JoinHandle<()>) {
    let server =
        RuntimeBridgeServer::bind(service, "127.0.0.1:0".parse().unwrap(), TOKEN, ORIGIN).unwrap();
    let addr = server.local_addr().unwrap();
    let handle = thread::spawn(move || {
        for _ in 0..request_count {
            server.serve_one().unwrap();
        }
    });
    (addr, handle)
}

fn spawn_server(request_count: usize) -> (SocketAddr, thread::JoinHandle<()>) {
    spawn_server_with(service(), request_count)
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

    let wrong_origin = post(addr, "/v1/plan", "https://evil.example", TOKEN, &plan_body);
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
    tampered_plan.steps[0]
        .action
        .args
        .push("tampered".to_owned());
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


fn recovery_service() -> RuntimeBridgeService {
    let mut implementations = ImplementationRegistry::new();
    let action = ProcessAction::new(
        env::current_exe().unwrap().display().to_string(),
        ["--list"],
    );
    implementations
        .register("impl:http-provider-a", action.clone())
        .unwrap();
    implementations
        .register("impl:http-provider-b", action)
        .unwrap();
    implementations
        .register_choice(
            "logical:http-provider",
            ["impl:http-provider-a", "impl:http-provider-b"],
            "impl:http-provider-a",
        )
        .unwrap();

    RuntimeBridgeService::new_with_recovery(
        implementations,
        RouteRegistry::new(),
        vec![RuntimeEndpoint::new(
            "runtime:http-agent-alt",
            RuntimeLocationClass::RuntimeAgent,
            ["impl:http-provider-a", "impl:http-provider-b"],
        )],
        "runtime:http-local",
    )
    .unwrap()
}

fn recovery_graph() -> Graph {
    let mut graph = Graph::new("graph:http-recovery");
    graph.blocks.push(Block {
        id: "block:http-recovery".to_owned(),
        label: "HTTP recovery".to_owned(),
        ports: Vec::new(),
        internal_graph_ref: None,
        implementation_ref: Some("logical:http-provider".to_owned()),
        definition_ref: None,
        source_anchor: None,
        extensions: Extensions::new(),
        diagnostics: Vec::new(),
    });
    graph
}

fn failed_trace(plan: &algoram_runtime::ExecutionPlan) -> ExecutionTrace {
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
            exit_code: Some(17),
            stdout: String::new(),
            stderr: "synthetic provider outage".to_owned(),
        }],
    }
}

#[test]
fn recovery_http_boundary_requires_auth_and_reuses_explicit_selection_for_run() {
    let (addr, server_thread) = spawn_server_with(recovery_service(), 5);
    let graph = recovery_graph();

    let plan_body = serde_json::to_string(&PlanRequest {
        graph: graph.clone(),
    })
    .unwrap();
    let planned = post(addr, "/v1/plan", ORIGIN, TOKEN, &plan_body);
    assert!(planned.starts_with("HTTP/1.1 200"));
    let default_preview: PlanResponse = serde_json::from_str(response_body(&planned)).unwrap();
    assert_eq!(
        default_preview.plan.steps[0].implementation_ref,
        "impl:http-provider-a"
    );

    let options_body = serde_json::to_string(&RecoveryOptionsRequest {
        graph: graph.clone(),
        expected_plan: default_preview.plan.clone(),
        trace: failed_trace(&default_preview.plan),
    })
    .unwrap();
    let unauthorized = post(
        addr,
        "/v1/recovery/options",
        ORIGIN,
        "wrong-token",
        &options_body,
    );
    assert!(unauthorized.starts_with("HTTP/1.1 401"));

    let options_response = post(addr, "/v1/recovery/options", ORIGIN, TOKEN, &options_body);
    assert!(options_response.starts_with("HTTP/1.1 200"));
    let options: RecoveryOptionsResponse =
        serde_json::from_str(response_body(&options_response)).unwrap();
    assert_eq!(options.implementation_candidates.len(), 1);
    assert!(options.implementation_candidates[0]
        .candidates
        .trusted_local
        .iter()
        .any(|candidate| candidate.implementation_ref == "impl:http-provider-b"));
    assert!(options.runtime_candidates[0]
        .candidates
        .iter()
        .any(|candidate| {
            candidate.candidate.runtime_ref == "runtime:http-agent-alt"
                && candidate.placement_validated
                && !candidate.executable_by_bridge
        }));

    let selections = RecoverySelections {
        route_overrides: Vec::new(),
        implementation_overrides: vec![ImplementationOverride {
            logical_ref: "logical:http-provider".to_owned(),
            implementation_ref: "impl:http-provider-b".to_owned(),
        }],
    };
    let recovery_plan_body = serde_json::to_string(&RecoveryPlanRequest {
        graph: graph.clone(),
        selections: selections.clone(),
    })
    .unwrap();
    let recovered = post(
        addr,
        "/v1/recovery/plan",
        ORIGIN,
        TOKEN,
        &recovery_plan_body,
    );
    assert!(recovered.starts_with("HTTP/1.1 200"));
    let recovery_preview: PlanResponse = serde_json::from_str(response_body(&recovered)).unwrap();
    assert_eq!(
        recovery_preview.plan.steps[0].implementation_ref,
        "impl:http-provider-b"
    );

    let run_body = serde_json::to_string(&RecoveryRunRequest {
        graph,
        expected_plan: recovery_preview.plan,
        allowed_implementation_refs: vec!["impl:http-provider-b".to_owned()],
        selections,
    })
    .unwrap();
    let executed = post(addr, "/v1/recovery/run", ORIGIN, TOKEN, &run_body);
    assert!(executed.starts_with("HTTP/1.1 200"));
    let run: RunResponse = serde_json::from_str(response_body(&executed)).unwrap();
    assert!(run.trace.succeeded());
    assert_eq!(
        run.trace.entries[0].implementation_ref,
        "impl:http-provider-b"
    );

    server_thread.join().unwrap();
}
