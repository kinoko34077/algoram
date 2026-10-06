use algoram_core::SourceAnchor;
use algoram_runtime::{
    ArgvBindingSource, ArgvPortBinding, DistributedExecutionError, DistributedRuntime,
    ExecutionPlan, ExecutionPolicy, ExecutionStep, ImplementationRegistry, PlacedExecutionPlan,
    ProcessAction, RuntimeAgent, RuntimeAgentRequest, RuntimeAgentResponse, RuntimeEndpoint,
    RuntimeLocationClass, StepPlacement, TraceEntry, TraceStatus,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const CHILD_MODE_ENV: &str = "ALGORAM_DISTRIBUTED_AGENT_CHILD";
const REQUEST_PATH_ENV: &str = "ALGORAM_DISTRIBUTED_AGENT_REQUEST_PATH";
const RESPONSE_PATH_ENV: &str = "ALGORAM_DISTRIBUTED_AGENT_RESPONSE_PATH";

fn anchor(artifact_id: &str, semantic_key: &str) -> SourceAnchor {
    SourceAnchor {
        artifact_id: artifact_id.to_owned(),
        start_byte: 0,
        end_byte: 16,
        start_line: Some(0),
        start_column: Some(0),
        end_line: Some(1),
        end_column: Some(0),
        semantic_key: Some(semantic_key.to_owned()),
    }
}

fn local_action() -> ProcessAction {
    ProcessAction::new(
        "python3",
        ["-c".to_owned(), "print('LOCAL_VALUE', end='')".to_owned()],
    )
}

fn remote_action() -> ProcessAction {
    ProcessAction::new(
        "python3",
        [
            "-c".to_owned(),
            "import sys; print('REMOTE:' + sys.argv[1], end='')".to_owned(),
        ],
    )
    .with_argv_ports(["value"])
}

fn split_plan(remote: ProcessAction) -> ExecutionPlan {
    ExecutionPlan {
        reference_graph_id: "graph:distributed-split".to_owned(),
        steps: vec![
            ExecutionStep {
                id: "step:local".to_owned(),
                implementation_ref: "impl:local".to_owned(),
                action: local_action(),
                origin_block_ids: vec!["block:local".to_owned()],
                source_anchors: vec![anchor("artifact:local", "python:local.main")],
                route_connector_ids: Vec::new(),
                argv_bindings: Vec::new(),
            },
            ExecutionStep {
                id: "step:agent".to_owned(),
                implementation_ref: "impl:agent".to_owned(),
                action: remote,
                origin_block_ids: vec!["block:agent".to_owned()],
                source_anchors: vec![anchor("artifact:agent", "python:agent.main")],
                route_connector_ids: vec!["route:local-agent:utf8".to_owned()],
                argv_bindings: vec![ArgvPortBinding {
                    target_port_id: "value".to_owned(),
                    source: ArgvBindingSource::StepStdout {
                        step_id: "step:local".to_owned(),
                    },
                }],
            },
        ],
    }
}

fn placed_split_plan(plan: ExecutionPlan) -> PlacedExecutionPlan {
    PlacedExecutionPlan::new(
        plan,
        [
            StepPlacement::new("step:local", "runtime:local"),
            StepPlacement::new("step:agent", "runtime:agent"),
        ],
    )
}

fn endpoints() -> Vec<RuntimeEndpoint> {
    vec![
        RuntimeEndpoint::new(
            "runtime:local",
            RuntimeLocationClass::LocalProcess,
            ["impl:local"],
        ),
        RuntimeEndpoint::new(
            "runtime:agent",
            RuntimeLocationClass::RuntimeAgent,
            ["impl:agent"],
        ),
    ]
}

fn local_trust() -> (ImplementationRegistry, ExecutionPolicy) {
    let mut implementations = ImplementationRegistry::new();
    implementations
        .register("impl:local", local_action())
        .unwrap();

    let mut policy = ExecutionPolicy::new();
    policy.allow("impl:local");
    (implementations, policy)
}

fn request_response_paths(tag: &str) -> (PathBuf, PathBuf) {
    let base = std::env::temp_dir();
    (
        base.join(format!(
            "algoram-distributed-request-{tag}-{}.json",
            std::process::id()
        )),
        base.join(format!(
            "algoram-distributed-response-{tag}-{}.json",
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
            "distributed_agent_child_entry",
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
fn local_stdout_crosses_agent_boundary_and_returns_one_attributed_trace() {
    let plan = split_plan(remote_action());
    let placed = placed_split_plan(plan.clone());
    let endpoint_set = endpoints();
    placed.validate(&endpoint_set).unwrap();

    let (local_implementations, local_policy) = local_trust();
    let expected_remote_action = remote_action();

    let trace = DistributedRuntime::execute_with_agent(
        &placed,
        &endpoint_set,
        &local_implementations,
        &local_policy,
        &[],
        |runtime_ref, request| {
            assert_eq!(runtime_ref, "runtime:agent");
            assert_eq!(request.plan.reference_graph_id, "graph:distributed-split");
            assert_eq!(request.plan.steps.len(), 1);
            assert_eq!(request.plan.steps[0].id, "step:agent");
            assert_eq!(request.plan.steps[0].action, expected_remote_action);
            assert_eq!(request.inputs.len(), 1);
            assert_eq!(request.inputs[0].value, "LOCAL_VALUE");
            assert!(matches!(
                &request.plan.steps[0].argv_bindings[0].source,
                ArgvBindingSource::ExternalPort { .. }
            ));
            Ok(run_child_agent(&request, "success"))
        },
    )
    .unwrap();

    assert!(trace.succeeded());
    assert_eq!(trace.reference_graph_id, "graph:distributed-split");
    assert_eq!(trace.entries.len(), 2);
    assert_eq!(trace.entries[0].runtime_ref, "runtime:local");
    assert_eq!(trace.entries[0].entry.step_id, "step:local");
    assert_eq!(trace.entries[0].entry.stdout, "LOCAL_VALUE");
    assert_eq!(trace.entries[1].runtime_ref, "runtime:agent");
    assert_eq!(trace.entries[1].entry.step_id, "step:agent");
    assert_eq!(trace.entries[1].entry.stdout, "REMOTE:LOCAL_VALUE");
    assert_eq!(
        trace.entries[1].entry.source_anchors[0]
            .semantic_key
            .as_deref(),
        Some("python:agent.main")
    );
    assert_eq!(
        trace.entries[1].entry.route_connector_ids,
        vec!["route:local-agent:utf8"]
    );

    assert_eq!(trace.transfers.len(), 1);
    let transfer = &trace.transfers[0];
    assert_eq!(transfer.source_step_id, "step:local");
    assert_eq!(transfer.target_step_id, "step:agent");
    assert_eq!(transfer.target_port_id, "value");
    assert_eq!(transfer.source_runtime_ref, "runtime:local");
    assert_eq!(transfer.target_runtime_ref, "runtime:agent");

    assert_eq!(placed.plan, plan);
    assert_eq!(placed.plan.steps[1].action, remote_action());
    assert!(matches!(
        &placed.plan.steps[1].argv_bindings[0].source,
        ArgvBindingSource::StepStdout { step_id } if step_id == "step:local"
    ));

    let serialized = serde_json::to_string(&trace).unwrap();
    let restored: algoram_runtime::DistributedExecutionTrace =
        serde_json::from_str(&serialized).unwrap();
    assert_eq!(restored, trace);
}

fn marker_remote_action(marker: &Path) -> ProcessAction {
    ProcessAction::new(
        "python3",
        [
            "-c".to_owned(),
            "from pathlib import Path; import sys; Path(sys.argv[1]).write_text(sys.argv[2])"
                .to_owned(),
            marker.display().to_string(),
        ],
    )
    .with_argv_ports(["value"])
}

#[test]
fn unavailable_agent_does_not_fall_back_to_local_execution() {
    let marker = std::env::temp_dir().join(format!(
        "algoram-distributed-agent-must-not-fallback-{}",
        std::process::id()
    ));
    let _ = fs::remove_file(&marker);

    let remote = marker_remote_action(&marker);
    let placed = placed_split_plan(split_plan(remote.clone()));
    let endpoint_set = endpoints();

    let (mut local_implementations, mut local_policy) = local_trust();
    local_implementations
        .register("impl:agent", remote)
        .unwrap();
    local_policy.allow("impl:agent");

    let error = DistributedRuntime::execute_with_agent(
        &placed,
        &endpoint_set,
        &local_implementations,
        &local_policy,
        &[],
        |runtime_ref, _request| {
            assert_eq!(runtime_ref, "runtime:agent");
            Err("simulated transport outage".to_owned())
        },
    )
    .unwrap_err();

    assert!(matches!(
        error,
        DistributedExecutionError::AgentUnavailable {
            runtime_ref,
            error,
        } if runtime_ref == "runtime:agent" && error == "simulated transport outage"
    ));
    assert!(!marker.exists());
}

#[test]
fn malformed_agent_trace_attribution_is_rejected() {
    let placed = placed_split_plan(split_plan(remote_action()));
    let endpoint_set = endpoints();
    let (local_implementations, local_policy) = local_trust();

    let error = DistributedRuntime::execute_with_agent(
        &placed,
        &endpoint_set,
        &local_implementations,
        &local_policy,
        &[],
        |_runtime_ref, request| {
            let submitted = &request.plan.steps[0];
            Ok(RuntimeAgentResponse::Executed {
                trace: algoram_runtime::ExecutionTrace {
                    reference_graph_id: request.plan.reference_graph_id.clone(),
                    entries: vec![TraceEntry {
                        step_id: submitted.id.clone(),
                        implementation_ref: "impl:forged".to_owned(),
                        status: TraceStatus::Succeeded,
                        origin_block_ids: submitted.origin_block_ids.clone(),
                        source_anchors: submitted.source_anchors.clone(),
                        route_connector_ids: submitted.route_connector_ids.clone(),
                        exit_code: Some(0),
                        stdout: "FORGED".to_owned(),
                        stderr: String::new(),
                    }],
                },
            })
        },
    )
    .unwrap_err();

    assert!(matches!(
        error,
        DistributedExecutionError::MalformedAgentResponse {
            runtime_ref,
            detail,
        } if runtime_ref == "runtime:agent"
            && detail.contains("trace attribution")
    ));
}

#[test]
#[ignore]
fn distributed_agent_child_entry() {
    if std::env::var(CHILD_MODE_ENV).as_deref() != Ok("1") {
        return;
    }

    let request_path = PathBuf::from(std::env::var(REQUEST_PATH_ENV).unwrap());
    let response_path = PathBuf::from(std::env::var(RESPONSE_PATH_ENV).unwrap());
    let request: RuntimeAgentRequest =
        serde_json::from_slice(&fs::read(request_path).unwrap()).unwrap();

    let mut trusted = ImplementationRegistry::new();
    trusted.register("impl:agent", remote_action()).unwrap();

    let mut policy = ExecutionPolicy::new();
    policy.allow("impl:agent");

    let response = RuntimeAgent::handle(request, &trusted, &policy);
    fs::write(response_path, serde_json::to_vec_pretty(&response).unwrap()).unwrap();
}
