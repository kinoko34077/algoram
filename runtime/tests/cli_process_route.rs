use algoram_core::{
    Block, Connection, Extensions, Graph, Port, PortChannel, PortDirection, PortRef,
};
use algoram_interop::{Connector, ContractId, RouteRegistry, RouteRequest, TransferMode};
use algoram_python_importer::{import_python, PythonImport};
use algoram_runtime::{
    ImplementationRegistry, Planner, ProcessAction, ProcessRuntime, TraceStatus,
};
use serde_json::json;
use std::fs;
use std::path::PathBuf;

const PROCESS_CONNECTOR_ID: &str = "text-utf8-to-process-argv-utf8";
const PROCESS_IMPL: &str = "fixture:cli-uppercase";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("runtime crate has repository parent")
        .to_path_buf()
}

fn import_fixture() -> PythonImport {
    let root = repo_root();
    let source_path = root.join("fixtures/cli-process/upper.py");
    let source = fs::read_to_string(&source_path).expect("CLI fixture source exists");

    import_python(
        "artifact:cli-process",
        "fixtures/cli-process/upper.py",
        &source,
    )
    .expect("CLI fixture imports")
}

fn port(id: &str, direction: PortDirection, contract: Option<&str>) -> Port {
    Port {
        id: id.to_owned(),
        direction,
        channel: PortChannel::Data,
        contract: contract.map(|value| json!(value)),
        extensions: Extensions::new(),
    }
}

fn input_block() -> Block {
    Block {
        id: "block:text-input".to_owned(),
        label: "Text input".to_owned(),
        ports: vec![port("value", PortDirection::Out, Some("text:utf8"))],
        internal_graph_ref: None,
        implementation_ref: None,
        definition_ref: None,
        source_anchor: None,
        extensions: Extensions::new(),
        diagnostics: Vec::new(),
    }
}

fn execution_graph(imported: &PythonImport) -> Graph {
    let mut graph = Graph::new("graph:cli-process-route");
    graph.label = Some("text:utf8 → process argv".to_owned());
    graph.source_artifacts = imported.root.source_artifacts.clone();

    let mut cli = imported.root.blocks[0].clone();
    cli.label = "CLI uppercase process".to_owned();
    cli.internal_graph_ref = None;
    cli.implementation_ref = Some(PROCESS_IMPL.to_owned());
    cli.ports = vec![port("argv", PortDirection::In, Some("process:argv:utf8"))];

    graph.blocks.extend([input_block(), cli.clone()]);
    graph.connections.push(Connection {
        id: "data:text-to-cli".to_owned(),
        source: PortRef {
            block_id: "block:text-input".to_owned(),
            port_id: "value".to_owned(),
        },
        target: PortRef {
            block_id: cli.id,
            port_id: "argv".to_owned(),
        },
        extensions: Extensions::new(),
    });

    graph.validate().expect("CLI execution graph validates");
    graph
}

fn route_registry() -> RouteRegistry {
    let mut routes = RouteRegistry::new();
    routes
        .register(
            Connector::new(
                PROCESS_CONNECTOR_ID,
                ContractId::from("text:utf8"),
                ContractId::from("process:argv:utf8"),
                "fixture:cli:argv",
            )
            .with_transfer(TransferMode::Copy),
        )
        .unwrap();
    routes
}

fn implementations(value: &str) -> ImplementationRegistry {
    let script = repo_root().join("fixtures/cli-process/upper.py");

    let mut implementations = ImplementationRegistry::new();
    implementations
        .register(
            PROCESS_IMPL,
            ProcessAction::new("python3", [script.display().to_string(), value.to_owned()]),
        )
        .unwrap();
    implementations
}

#[test]
fn process_route_is_non_abi_and_inspectable() {
    let routes = route_registry();
    assert_eq!(routes.connectors().len(), 1);

    let connector = &routes.connectors()[0];
    assert_eq!(connector.id, PROCESS_CONNECTOR_ID);
    assert_eq!(connector.source.as_str(), "text:utf8");
    assert_eq!(connector.target.as_str(), "process:argv:utf8");
    assert!(!connector.source.as_str().contains("abi"));
    assert!(!connector.target.as_str().contains("abi"));

    let resolution = routes
        .resolve(&RouteRequest::automatic("text:utf8", "process:argv:utf8"))
        .unwrap();
    let route = resolution.route().expect("CLI process route resolves");

    assert_eq!(route.connector_ids(), vec![PROCESS_CONNECTOR_ID]);

    let inspection = route.to_inspection_graph().unwrap();
    assert_eq!(inspection.blocks.len(), 1);
    assert!(inspection.connections.is_empty());
    assert_eq!(
        inspection.blocks[0].extensions["interop"]["connector_id"],
        json!(PROCESS_CONNECTOR_ID)
    );
}

#[test]
fn cli_process_success_preserves_route_and_python_source_anchor() {
    let imported = import_fixture();
    let graph = execution_graph(&imported);
    let routes = route_registry();
    let implementations = implementations("algoram route");

    let plan = Planner::lower(&graph, &implementations, &routes).unwrap();
    assert_eq!(plan.steps.len(), 1);

    let step = &plan.steps[0];
    assert_eq!(step.route_connector_ids, vec![PROCESS_CONNECTOR_ID]);
    assert_eq!(step.source_anchors.len(), 1);
    assert_eq!(step.source_anchors[0].artifact_id, "artifact:cli-process");
    assert_eq!(
        step.origin_block_ids,
        vec!["python:artifact:cli-process:file".to_owned()]
    );

    let trace = ProcessRuntime::execute(&plan);
    assert!(trace.succeeded());
    assert_eq!(trace.entries.len(), 1);
    assert_eq!(trace.entries[0].status, TraceStatus::Succeeded);
    assert_eq!(trace.entries[0].stdout.trim(), "ALGORAM ROUTE");
    assert_eq!(
        trace.entries[0].route_connector_ids,
        vec![PROCESS_CONNECTOR_ID]
    );
    assert_eq!(trace.entries[0].source_anchors, step.source_anchors);
}

#[test]
fn cli_process_failure_preserves_route_and_python_source_anchor() {
    let imported = import_fixture();
    let graph = execution_graph(&imported);
    let routes = route_registry();
    let implementations = implementations("__fail__");

    let plan = Planner::lower(&graph, &implementations, &routes).unwrap();
    let step = plan.steps[0].clone();

    let trace = ProcessRuntime::execute(&plan);
    let failure = trace.failed_entry().expect("CLI failure is observed");

    assert_eq!(failure.status, TraceStatus::Failed);
    assert_eq!(failure.exit_code, Some(17));
    assert!(failure.stderr.contains("cli failure"));
    assert_eq!(failure.route_connector_ids, vec![PROCESS_CONNECTOR_ID]);
    assert_eq!(failure.source_anchors, step.source_anchors);
}
