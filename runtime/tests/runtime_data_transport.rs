use algoram_core::{
    Block, Connection, Extensions, Graph, Port, PortChannel, PortDirection, PortRef, SourceAnchor,
    SourceArtifact,
};
use algoram_interop::{Connector, ContractId, RouteRegistry, TransferMode};
use algoram_runtime::{
    ExecutionPlan, ImplementationRegistry, Planner, ProcessAction, ProcessRuntime, RuntimeInput,
    TraceStatus,
};
use serde_json::json;
use std::path::PathBuf;

const GRAPH_ID: &str = "graph:runtime-data-transport";
const EXTERNAL_BLOCK: &str = "block:external";
const UPPERCASE_BLOCK: &str = "block:uppercase";
const PREFIX_BLOCK: &str = "block:prefix";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("runtime crate has repository parent")
        .to_path_buf()
}

fn port(
    id: &str,
    direction: PortDirection,
    channel: PortChannel,
    contract: Option<&str>,
) -> Port {
    Port {
        id: id.to_owned(),
        direction,
        channel,
        contract: contract.map(|value| json!(value)),
        extensions: Extensions::new(),
    }
}

fn block(id: &str, implementation_ref: Option<&str>) -> Block {
    Block {
        id: id.to_owned(),
        label: id.to_owned(),
        ports: Vec::new(),
        internal_graph_ref: None,
        implementation_ref: implementation_ref.map(str::to_owned),
        definition_ref: None,
        source_anchor: None,
        extensions: Extensions::new(),
        diagnostics: Vec::new(),
    }
}

fn data_connection(
    id: &str,
    source_block: &str,
    source_port: &str,
    target_block: &str,
    target_port: &str,
) -> Connection {
    Connection {
        id: id.to_owned(),
        source: PortRef {
            block_id: source_block.to_owned(),
            port_id: source_port.to_owned(),
        },
        target: PortRef {
            block_id: target_block.to_owned(),
            port_id: target_port.to_owned(),
        },
        extensions: Extensions::new(),
    }
}

fn whole_anchor(artifact_id: &str, source: &str, semantic_key: &str) -> SourceAnchor {
    SourceAnchor {
        artifact_id: artifact_id.to_owned(),
        start_byte: 0,
        end_byte: source.len() as u64,
        start_line: Some(0),
        start_column: Some(0),
        end_line: None,
        end_column: None,
        semantic_key: Some(semantic_key.to_owned()),
    }
}

fn reference_graph() -> Graph {
    const UPPERCASE_SOURCE: &str = include_str!("../../fixtures/runtime-data/uppercase.py");
    const PREFIX_SOURCE: &str = include_str!("../../fixtures/runtime-data/prefix.py");

    let uppercase_artifact = SourceArtifact {
        id: "artifact:runtime-uppercase".to_owned(),
        origin: "fixtures/runtime-data/uppercase.py".to_owned(),
        language: "python".to_owned(),
        revision: Some("phase4-reference".to_owned()),
        content_hash: None,
        repository_origin: None,
        license: None,
    };
    let prefix_artifact = SourceArtifact {
        id: "artifact:runtime-prefix".to_owned(),
        origin: "fixtures/runtime-data/prefix.py".to_owned(),
        language: "python".to_owned(),
        revision: Some("phase4-reference".to_owned()),
        content_hash: None,
        repository_origin: None,
        license: None,
    };

    let mut external = block(EXTERNAL_BLOCK, None);
    external.ports.push(port(
        "text",
        PortDirection::Out,
        PortChannel::Data,
        Some("external:utf8"),
    ));

    let mut uppercase = block(UPPERCASE_BLOCK, Some("fixture:uppercase"));
    uppercase.ports.extend([
        port(
            "text",
            PortDirection::In,
            PortChannel::Data,
            Some("process:argv:utf8"),
        ),
        port(
            "stdout",
            PortDirection::Out,
            PortChannel::Data,
            Some("process:stdout:utf8"),
        ),
        port("flow_out", PortDirection::Out, PortChannel::Flow, None),
    ]);
    uppercase.source_anchor = Some(whole_anchor(
        &uppercase_artifact.id,
        UPPERCASE_SOURCE,
        "python:runtime-data.uppercase",
    ));

    let mut prefix = block(PREFIX_BLOCK, Some("fixture:prefix"));
    prefix.ports.extend([
        port("flow_in", PortDirection::In, PortChannel::Flow, None),
        port(
            "text",
            PortDirection::In,
            PortChannel::Data,
            Some("process:argv:utf8"),
        ),
    ]);
    prefix.source_anchor = Some(whole_anchor(
        &prefix_artifact.id,
        PREFIX_SOURCE,
        "python:runtime-data.prefix",
    ));

    let mut graph = Graph::new(GRAPH_ID);
    graph.source_artifacts.extend([uppercase_artifact, prefix_artifact]);
    graph.blocks.extend([external, uppercase, prefix]);
    graph.connections.extend([
        data_connection(
            "data:external-uppercase",
            EXTERNAL_BLOCK,
            "text",
            UPPERCASE_BLOCK,
            "text",
        ),
        data_connection(
            "data:uppercase-prefix",
            UPPERCASE_BLOCK,
            "stdout",
            PREFIX_BLOCK,
            "text",
        ),
        Connection {
            id: "flow:uppercase-prefix".to_owned(),
            source: PortRef {
                block_id: UPPERCASE_BLOCK.to_owned(),
                port_id: "flow_out".to_owned(),
            },
            target: PortRef {
                block_id: PREFIX_BLOCK.to_owned(),
                port_id: "flow_in".to_owned(),
            },
            extensions: Extensions::new(),
        },
    ]);
    graph
}

fn routes() -> RouteRegistry {
    let mut routes = RouteRegistry::new();
    routes
        .register(
            Connector::new(
                "external-to-process-argv-utf8",
                ContractId::from("external:utf8"),
                ContractId::from("process:argv:utf8"),
                "fixture:external-argv-utf8",
            )
            .with_transfer(TransferMode::Copy),
        )
        .unwrap();
    routes
        .register(
            Connector::new(
                "stdout-to-process-argv-utf8",
                ContractId::from("process:stdout:utf8"),
                ContractId::from("process:argv:utf8"),
                "fixture:stdout-argv-utf8",
            )
            .with_transfer(TransferMode::Copy),
        )
        .unwrap();
    routes
}

fn implementations() -> ImplementationRegistry {
    let root = repo_root();
    let uppercase = root.join("fixtures/runtime-data/uppercase.py");
    let prefix = root.join("fixtures/runtime-data/prefix.py");

    let mut implementations = ImplementationRegistry::new();
    implementations
        .register(
            "fixture:uppercase",
            ProcessAction::new("python3", [uppercase.display().to_string()])
                .with_argv_ports(["text"]),
        )
        .unwrap();
    implementations
        .register(
            "fixture:prefix",
            ProcessAction::new("python3", [prefix.display().to_string()])
                .with_argv_ports(["text"]),
        )
        .unwrap();
    implementations
}

fn replayable_plan() -> ExecutionPlan {
    let graph = reference_graph();
    let implementations = implementations();
    let routes = routes();

    let planned = Planner::lower(&graph, &implementations, &routes).unwrap();

    assert_eq!(planned.reference_graph_id, GRAPH_ID);
    assert_eq!(planned.steps.len(), 2);
    assert_eq!(
        planned.steps[0].route_connector_ids,
        vec!["external-to-process-argv-utf8"]
    );
    assert_eq!(
        planned.steps[1].route_connector_ids,
        vec!["stdout-to-process-argv-utf8"]
    );

    let json = serde_json::to_string_pretty(&planned).unwrap();
    serde_json::from_str(&json).unwrap()
}

fn runtime_input(value: &str) -> RuntimeInput {
    RuntimeInput::new(EXTERNAL_BLOCK, "text", value)
}

#[test]
fn same_deserialized_plan_replays_two_external_values_through_real_processes() {
    let plan = replayable_plan();

    let first = ProcessRuntime::execute_with_inputs(&plan, &[runtime_input("algoram")]);
    assert!(first.succeeded());
    assert_eq!(first.reference_graph_id, GRAPH_ID);
    assert_eq!(first.entries.len(), 2);
    assert_eq!(first.entries[0].stdout, "ALGORAM");
    assert_eq!(first.entries[1].stdout, "RESULT:ALGORAM");
    assert_eq!(first.entries[0].origin_block_ids, vec![UPPERCASE_BLOCK]);
    assert_eq!(first.entries[1].origin_block_ids, vec![PREFIX_BLOCK]);
    assert_eq!(
        first.entries[0].route_connector_ids,
        vec!["external-to-process-argv-utf8"]
    );
    assert_eq!(
        first.entries[1].route_connector_ids,
        vec!["stdout-to-process-argv-utf8"]
    );
    assert_eq!(
        first.entries[0].source_anchors[0].artifact_id,
        "artifact:runtime-uppercase"
    );
    assert_eq!(
        first.entries[1].source_anchors[0].artifact_id,
        "artifact:runtime-prefix"
    );

    let second = ProcessRuntime::execute_with_inputs(&plan, &[runtime_input("blocks")]);
    assert!(second.succeeded());
    assert_eq!(second.entries[0].stdout, "BLOCKS");
    assert_eq!(second.entries[1].stdout, "RESULT:BLOCKS");
}

#[test]
fn missing_external_input_fails_before_launch_and_later_step_is_not_run() {
    let plan = replayable_plan();

    let trace = ProcessRuntime::execute_with_inputs(&plan, &[]);

    assert!(!trace.succeeded());
    assert_eq!(trace.reference_graph_id, GRAPH_ID);
    assert_eq!(trace.entries.len(), 2);
    assert_eq!(trace.entries[0].status, TraceStatus::Failed);
    assert_eq!(trace.entries[0].exit_code, None);
    assert!(trace.entries[0].stdout.is_empty());
    assert!(trace.entries[0]
        .stderr
        .contains("missing runtime input for external Port 'block:external.text'"));
    assert_eq!(trace.entries[0].origin_block_ids, vec![UPPERCASE_BLOCK]);
    assert_eq!(
        trace.entries[0].source_anchors[0].artifact_id,
        "artifact:runtime-uppercase"
    );
    assert_eq!(
        trace.entries[0].route_connector_ids,
        vec!["external-to-process-argv-utf8"]
    );
    assert_eq!(trace.entries[1].status, TraceStatus::NotRun);
}
