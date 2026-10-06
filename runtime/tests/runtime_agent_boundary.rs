use algoram_runtime::{
    ExecutionPlan, ExecutionPolicy, ExecutionStep, ImplementationRegistry, PlacedExecutionPlan,
    ProcessAction, RuntimeAgent, RuntimeAgentRequest, RuntimeAgentResponse, RuntimeEndpoint,
    RuntimeLocationClass, StepPlacement, TraceStatus,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const CHILD_MODE_ENV: &str = "ALGORAM_RUNTIME_AGENT_CHILD";
const REQUEST_PATH_ENV: &str = "ALGORAM_RUNTIME_AGENT_REQUEST_PATH";
const RESPONSE_PATH_ENV: &str = "ALGORAM_RUNTIME_AGENT_RESPONSE_PATH";

fn trusted_agent_action() -> ProcessAction {
    ProcessAction::new(
        "python3",
        ["-c".to_owned(), "print('AGENT_BOUNDARY_OK')".to_owned()],
    )
}

fn agent_plan(action: ProcessAction) -> ExecutionPlan {
    ExecutionPlan {
        reference_graph_id: "graph:agent-boundary".to_owned(),
        steps: vec![ExecutionStep {
            id: "step:agent-boundary".to_owned(),
            implementation_ref: "impl:agent-boundary".to_owned(),
            action,
            origin_block_ids: vec!["block:agent-boundary".to_owned()],
            source_anchors: Vec::new(),
            route_connector_ids: Vec::new(),
            argv_bindings: Vec::new(),
        }],
    }
}

fn agent_endpoint() -> RuntimeEndpoint {
    RuntimeEndpoint::new(
        "runtime:agent-boundary",
        RuntimeLocationClass::RuntimeAgent,
        ["impl:agent-boundary"],
    )
}

fn request_response_paths(tag: &str) -> (PathBuf, PathBuf) {
    let base = std::env::temp_dir();
    (
        base.join(format!(
            "algoram-agent-request-{tag}-{}.json",
            std::process::id()
        )),
        base.join(format!(
            "algoram-agent-response-{tag}-{}.json",
            std::process::id()
        )),
    )
}

fn run_child_agent(request: &RuntimeAgentRequest, tag: &str) -> RuntimeAgentResponse {
    let (request_path, response_path) = request_response_paths(tag);
    let _ = fs::remove_file(&request_path);
    let _ = fs::remove_file(&response_path);

    fs::write(&request_path, serde_json::to_vec_pretty(request).unwrap()).unwrap();

    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "runtime_agent_child_entry",
            "--ignored",
            "--nocapture",
        ])
        .env(CHILD_MODE_ENV, "1")
        .env(REQUEST_PATH_ENV, &request_path)
        .env(RESPONSE_PATH_ENV, &response_path)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "child agent failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let response: RuntimeAgentResponse =
        serde_json::from_slice(&fs::read(&response_path).unwrap()).unwrap();

    let _ = fs::remove_file(&request_path);
    let _ = fs::remove_file(&response_path);
    response
}

#[test]
fn placed_execution_unit_crosses_independent_agent_process_boundary() {
    let plan = agent_plan(trusted_agent_action());
    let placed = PlacedExecutionPlan::new(
        plan.clone(),
        [StepPlacement::new(
            "step:agent-boundary",
            "runtime:agent-boundary",
        )],
    );
    placed.validate(&[agent_endpoint()]).unwrap();

    let request = RuntimeAgentRequest::new(plan, Vec::new());
    let serialized = serde_json::to_string(&request).unwrap();
    let request_round_trip: RuntimeAgentRequest = serde_json::from_str(&serialized).unwrap();
    assert_eq!(request_round_trip, request);
    assert!(!serialized.contains("ImplementationRegistry"));
    assert!(!serialized.contains("ExecutionPolicy"));
    assert!(!serialized.contains("RouteRegistry"));

    let response = run_child_agent(&request_round_trip, "success");
    let response_json = serde_json::to_string(&response).unwrap();
    let response_round_trip: RuntimeAgentResponse = serde_json::from_str(&response_json).unwrap();
    assert_eq!(response_round_trip, response);

    match response_round_trip {
        RuntimeAgentResponse::Executed { trace } => {
            assert!(trace.succeeded());
            assert_eq!(trace.reference_graph_id, "graph:agent-boundary");
            assert_eq!(trace.entries.len(), 1);
            assert_eq!(trace.entries[0].status, TraceStatus::Succeeded);
            assert_eq!(
                trace.entries[0].implementation_ref,
                "impl:agent-boundary"
            );
            assert_eq!(
                trace.entries[0].origin_block_ids,
                vec!["block:agent-boundary"]
            );
            assert_eq!(trace.entries[0].stdout.trim(), "AGENT_BOUNDARY_OK");
        }
        RuntimeAgentResponse::Rejected { error } => {
            panic!("agent unexpectedly rejected trusted request: {error}");
        }
    }
}

fn marker_action(marker: &Path) -> ProcessAction {
    ProcessAction::new(
        "python3",
        [
            "-c".to_owned(),
            "from pathlib import Path; import sys; Path(sys.argv[1]).write_text('TAMPERED')"
                .to_owned(),
            marker.display().to_string(),
        ],
    )
}

#[test]
fn independent_agent_rejects_tampered_action_before_side_effect() {
    let marker = std::env::temp_dir().join(format!(
        "algoram-agent-tampered-must-not-run-{}",
        std::process::id()
    ));
    let _ = fs::remove_file(&marker);

    let request = RuntimeAgentRequest::new(agent_plan(marker_action(&marker)), Vec::new());
    assert_eq!(
        request.plan.steps[0].implementation_ref,
        "impl:agent-boundary"
    );

    let response = run_child_agent(&request, "tampered");
    match response {
        RuntimeAgentResponse::Rejected { error } => {
            assert!(error.contains("does not match trusted local implementation"));
        }
        RuntimeAgentResponse::Executed { trace } => {
            panic!("tampered request unexpectedly executed: {trace:?}");
        }
    }

    assert!(!marker.exists());
}

#[test]
#[ignore]
fn runtime_agent_child_entry() {
    if std::env::var(CHILD_MODE_ENV).as_deref() != Ok("1") {
        return;
    }

    let request_path = PathBuf::from(std::env::var(REQUEST_PATH_ENV).unwrap());
    let response_path = PathBuf::from(std::env::var(RESPONSE_PATH_ENV).unwrap());
    let request: RuntimeAgentRequest =
        serde_json::from_slice(&fs::read(request_path).unwrap()).unwrap();

    let mut trusted = ImplementationRegistry::new();
    trusted
        .register("impl:agent-boundary", trusted_agent_action())
        .unwrap();

    let mut policy = ExecutionPolicy::new();
    policy.allow("impl:agent-boundary");

    let response = RuntimeAgent::handle(request, &trusted, &policy);
    fs::write(response_path, serde_json::to_vec_pretty(&response).unwrap()).unwrap();
}
