use algoram_core::{
    Block, Connection, Extensions, Graph, Port, PortChannel, PortDirection, PortRef, SourceAnchor,
    SourceArtifact,
};
use algoram_interop::{Connector, ContractId, RouteRegistry, TransferMode};
use algoram_runtime::{
    ExecutionPlan, ImplementationRegistry, Planner, ProcessAction, ProcessRuntime, TraceStatus,
};
use serde_json::json;

const ROUTE_ID: &str = "text-to-replay-process-argv";
const IMPLEMENTATION_REF: &str = "fixture:replay-process";
const SOURCE_ARTIFACT_ID: &str = "artifact:plan-replay";
const SOURCE_TEXT: &str = "print('replayed-plan')\n";

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

fn structural_block(id: &str) -> Block {
    Block {
        id: id.to_owned(),
        label: id.to_owned(),
        ports: Vec::new(),
        internal_graph_ref: None,
        implementation_ref: None,
        definition_ref: None,
        source_anchor: None,
        extensions: Extensions::new(),
        diagnostics: Vec::new(),
    }
}

fn replay_graph() -> Graph {
    let mut graph = Graph::new("graph:plan-replay");
    graph.label = Some("Execution Plan JSON replay proof".to_owned());
    graph.source_artifacts.push(SourceArtifact {
        id: SOURCE_ARTIFACT_ID.to_owned(),
        origin: "runtime/tests/plan_replay.rs".to_owned(),
        language: "python".to_owned(),
        revision: Some("phase4-replay-proof".to_owned()),
        content_hash: None,
        repository_origin: None,
        license: None,
    });

    let mut input = structural_block("block:replay-input");
    input.ports.push(port(
        "value",
        PortDirection::Out,
        PortChannel::Data,
        Some("text:utf8"),
    ));

    let mut process = structural_block("block:replay-process");
    process.label = "Replay process".to_owned();
    process.implementation_ref = Some(IMPLEMENTATION_REF.to_owned());
    process.ports.push(port(
        "value",
        PortDirection::In,
        PortChannel::Data,
        Some("process:argv:utf8"),
    ));
    process.source_anchor = Some(SourceAnchor {
        artifact_id: SOURCE_ARTIFACT_ID.to_owned(),
        start_byte: 0,
        end_byte: SOURCE_TEXT.len() as u64,
        start_line: Some(0),
        start_column: Some(0),
        end_line: Some(0),
        end_column: Some(SOURCE_TEXT.trim_end().len() as u32),
        semantic_key: Some("phase4:replay-process".to_owned()),
    });

    graph.blocks.extend([input, process]);
    graph.connections.push(Connection {
        id: "data:replay-input-process".to_owned(),
        source: PortRef {
            block_id: "block:replay-input".to_owned(),
            port_id: "value".to_owned(),
        },
        target: PortRef {
            block_id: "block:replay-process".to_owned(),
            port_id: "value".to_owned(),
        },
        extensions: Extensions::new(),
    });

    graph.validate().expect("replay graph validates");
    graph
}

fn replay_routes() -> RouteRegistry {
    let mut routes = RouteRegistry::new();
    routes
        .register(
            Connector::new(
                ROUTE_ID,
                ContractId::from("text:utf8"),
                ContractId::from("process:argv:utf8"),
                "fixture:text-to-process-argv",
            )
            .with_transfer(TransferMode::Copy),
        )
        .unwrap();
    routes
}

fn replay_implementations() -> ImplementationRegistry {
    let mut implementations = ImplementationRegistry::new();
    implementations
        .register(
            IMPLEMENTATION_REF,
            ProcessAction::new(
                "python3",
                ["-c".to_owned(), SOURCE_TEXT.trim_end().to_owned()],
            ),
        )
        .unwrap();
    implementations
}

#[test]
fn serialized_execution_plan_replays_without_planner_inputs() {
    let (serialized, expected_plan) = {
        let graph = replay_graph();
        let implementations = replay_implementations();
        let routes = replay_routes();

        let plan = Planner::lower(&graph, &implementations, &routes)
            .expect("reference graph lowers to an execution plan");

        assert_eq!(plan.reference_graph_id, "graph:plan-replay");
        assert_eq!(plan.steps.len(), 1);

        let step = &plan.steps[0];
        assert_eq!(step.implementation_ref, IMPLEMENTATION_REF);
        assert_eq!(step.origin_block_ids, vec!["block:replay-process"]);
        assert_eq!(step.route_connector_ids, vec![ROUTE_ID]);
        assert_eq!(step.source_anchors.len(), 1);
        assert_eq!(step.source_anchors[0].artifact_id, SOURCE_ARTIFACT_ID);
        assert_eq!(
            step.source_anchors[0].semantic_key.as_deref(),
            Some("phase4:replay-process")
        );

        (
            serde_json::to_string_pretty(&plan).expect("plan serializes to JSON"),
            plan,
        )
    };

    // The Graph, ImplementationRegistry, and RouteRegistry no longer exist here.
    // Replay uses only the serialized execution artifact.
    let replayed: ExecutionPlan =
        serde_json::from_str(&serialized).expect("plan deserializes from JSON");

    assert_eq!(replayed, expected_plan);

    let trace = ProcessRuntime::execute(&replayed);

    assert!(trace.succeeded());
    assert_eq!(trace.reference_graph_id, "graph:plan-replay");
    assert_eq!(trace.entries.len(), 1);

    let entry = &trace.entries[0];
    assert_eq!(entry.status, TraceStatus::Succeeded);
    assert_eq!(entry.stdout.trim(), "replayed-plan");
    assert!(entry.stderr.is_empty());
    assert_eq!(entry.origin_block_ids, vec!["block:replay-process"]);
    assert_eq!(entry.route_connector_ids, vec![ROUTE_ID]);
    assert_eq!(entry.source_anchors, replayed.steps[0].source_anchors);
}
