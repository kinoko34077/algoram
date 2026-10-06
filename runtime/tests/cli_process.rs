use algoram_core::{
    Block, Connection, Extensions, Graph, Port, PortChannel, PortDirection, PortRef,
};
use algoram_interop::{Connector, ContractId, RouteRegistry, RouteRequest, TransferMode};
use algoram_runtime::{
    ImplementationRegistry, Planner, ProcessAction, ProcessRuntime, TraceStatus,
};
use serde_json::json;
use std::path::PathBuf;

const PROCESS_CONNECTOR_ID: &str = "int32-to-cli-argv-int32";
const SOURCE_CONTRACT: &str = "algoram:value:int32";
const PROCESS_CONTRACT: &str = "process:argv:int32";
const PROCESS_IMPL: &str = "fixture:cli-process-triple";

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

fn route_registry() -> RouteRegistry {
    let mut routes = RouteRegistry::new();
    routes
        .register(
            Connector::new(
                PROCESS_CONNECTOR_ID,
                ContractId::from(SOURCE_CONTRACT),
                ContractId::from(PROCESS_CONTRACT),
                "fixture:cli-argv-int32",
            )
            .with_transfer(TransferMode::Copy),
        )
        .unwrap();
    routes
}

fn execution_graph() -> Graph {
    let mut graph = Graph::new("graph:cli-process-proof");
    graph.label = Some("Algoram value → CLI process argument".to_owned());

    let mut input = structural_block("block:algoram-int32");
    input.ports.push(port(
        "value",
        PortDirection::Out,
        PortChannel::Data,
        Some(SOURCE_CONTRACT),
    ));

    let mut process = structural_block("block:cli-triple-process");
    process.label = "External CLI triple process".to_owned();
    process.implementation_ref = Some(PROCESS_IMPL.to_owned());
    process.ports.push(port(
        "value",
        PortDirection::In,
        PortChannel::Data,
        Some(PROCESS_CONTRACT),
    ));

    graph.blocks.extend([input, process]);
    graph.connections.push(Connection {
        id: "data:algoram-value-to-cli".to_owned(),
        source: PortRef {
            block_id: "block:algoram-int32".to_owned(),
            port_id: "value".to_owned(),
        },
        target: PortRef {
            block_id: "block:cli-triple-process".to_owned(),
            port_id: "value".to_owned(),
        },
        extensions: Extensions::new(),
    });

    graph.validate().expect("CLI proof graph validates");
    graph
}

fn implementations(value: i32) -> ImplementationRegistry {
    let script = repo_root().join("fixtures/cli-process/triple.py");
    let mut implementations = ImplementationRegistry::new();
    implementations
        .register(
            PROCESS_IMPL,
            ProcessAction::new(
                "python3",
                [script.display().to_string(), value.to_string()],
            ),
        )
        .unwrap();
    implementations
}

#[test]
fn cli_process_boundary_is_an_ordinary_inspectable_route() {
    let routes = route_registry();
    assert_eq!(routes.connectors().len(), 1);

    let connector = &routes.connectors()[0];
    assert_eq!(connector.id, PROCESS_CONNECTOR_ID);
    assert_eq!(connector.source.as_str(), SOURCE_CONTRACT);
    assert_eq!(connector.target.as_str(), PROCESS_CONTRACT);
    assert_eq!(connector.implementation_ref, "fixture:cli-argv-int32");

    let resolution = routes
        .resolve(&RouteRequest::automatic(SOURCE_CONTRACT, PROCESS_CONTRACT))
        .unwrap();
    let route = resolution.route().expect("CLI process route resolves");

    assert_eq!(route.connector_ids(), vec![PROCESS_CONNECTOR_ID]);
    assert!(route.is_direct());

    let inspection = route.to_inspection_graph().unwrap();
    inspection.validate().unwrap();
    assert_eq!(inspection.blocks.len(), 1);
    assert!(inspection.connections.is_empty());
    assert_eq!(
        inspection.blocks[0].extensions["interop"]["connector_id"],
        json!(PROCESS_CONNECTOR_ID)
    );
    assert_eq!(
        inspection.blocks[0].ports[0].contract,
        Some(json!(SOURCE_CONTRACT))
    );
    assert_eq!(
        inspection.blocks[0].ports[1].contract,
        Some(json!(PROCESS_CONTRACT))
    );
}

#[test]
fn planner_and_process_runtime_preserve_cli_route_and_stdout() {
    let graph = execution_graph();
    let routes = route_registry();
    let implementations = implementations(14);

    let plan = Planner::lower(&graph, &implementations, &routes).unwrap();
    assert_eq!(plan.steps.len(), 1);

    let step = &plan.steps[0];
    assert_eq!(step.implementation_ref, PROCESS_IMPL);
    assert_eq!(
        step.route_connector_ids,
        vec![PROCESS_CONNECTOR_ID.to_owned()]
    );

    let trace = ProcessRuntime::execute(&plan);
    assert!(trace.succeeded());
    assert_eq!(trace.entries.len(), 1);

    let entry = &trace.entries[0];
    assert_eq!(entry.status, TraceStatus::Succeeded);
    assert_eq!(entry.stdout.trim(), "42");
    assert_eq!(
        entry.route_connector_ids,
        vec![PROCESS_CONNECTOR_ID.to_owned()]
    );
    assert!(entry.stderr.is_empty());
}
