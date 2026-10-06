use algoram_core::{
    Block, Connection, Extensions, Graph, Port, PortChannel, PortDirection, PortRef,
};
use algoram_interop::{Connector, ContractId, RouteRegistry, RouteRequest, TransferMode};
use algoram_runtime::{
    ImplementationRegistry, Planner, ProcessAction, ProcessRuntime, TraceStatus,
};
use algoram_rust_importer::{import_rust, RustImport};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

const PYTHON_C_ABI_CONNECTOR_ID: &str = "python-ctypes-c-abi-int32";
const RUST_C_ABI_CONNECTOR_ID: &str = "c-abi-call-rust-algoram-checked-triple";
const RUST_FUNCTION_CONTRACT: &str = "rust:extern-c:function:algoram_checked_triple:int32";
const BUILD_IMPL: &str = "fixture:build-rust-cdylib";
const INVOKE_IMPL: &str = "fixture:invoke-rust-through-python-ctypes";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("runtime crate has repository parent")
        .to_path_buf()
}

fn port(id: &str, direction: PortDirection, channel: PortChannel, contract: Option<&str>) -> Port {
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

fn connection(
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

fn import_fixture() -> RustImport {
    let root = repo_root();
    let source_path = root.join("fixtures/python-rust-cabi/bridge.rs");
    let source = fs::read_to_string(&source_path).expect("Rust fixture source exists");
    import_rust(
        "artifact:python-rust-cabi",
        "fixtures/python-rust-cabi/bridge.rs",
        &source,
    )
    .expect("Rust fixture imports")
}

fn rust_syntax(block: &Block) -> &serde_json::Value {
    &block.extensions["rust"]["syntax"]
}

fn imported_function(imported: &RustImport) -> Block {
    imported
        .root
        .blocks
        .iter()
        .chain(
            imported
                .nested_graphs
                .values()
                .flat_map(|graph| graph.blocks.iter()),
        )
        .find(|block| {
            rust_syntax(block)["kind"] == json!("function_item")
                && rust_syntax(block)["name"] == json!("algoram_checked_triple")
        })
        .expect("imported Rust function exists")
        .clone()
}

fn route_registry() -> RouteRegistry {
    let mut routes = RouteRegistry::new();

    routes
        .register(
            Connector::new(
                PYTHON_C_ABI_CONNECTOR_ID,
                ContractId::from("python:ctypes:c_int"),
                ContractId::from("c:abi:int32"),
                "fixture:python-c-ctypes",
            )
            .with_transfer(TransferMode::Copy),
        )
        .unwrap();

    routes
        .register(
            Connector::new(
                RUST_C_ABI_CONNECTOR_ID,
                ContractId::from("c:abi:int32"),
                ContractId::from(RUST_FUNCTION_CONTRACT),
                "fixture:rust:extern-c:algoram_checked_triple",
            )
            .with_transfer(TransferMode::Copy),
        )
        .unwrap();

    routes
}

fn execution_graph(imported: &RustImport) -> Graph {
    let imported_function = imported_function(imported);
    let function_anchor = imported_function
        .source_anchor
        .clone()
        .expect("imported function is source anchored");

    let mut graph = Graph::new("graph:python-rust-cabi");
    graph.label = Some("Python ctypes → C ABI → Rust extern C".to_owned());
    graph.source_artifacts = imported.root.source_artifacts.clone();

    let mut input = structural_block("block:python-input");
    input.ports.push(port(
        "value",
        PortDirection::Out,
        PortChannel::Data,
        Some("python:ctypes:c_int"),
    ));

    let mut build = structural_block("block:build-rust");
    build.implementation_ref = Some(BUILD_IMPL.to_owned());
    build.ports.push(port(
        "flow_out",
        PortDirection::Out,
        PortChannel::Flow,
        None,
    ));
    build.source_anchor = imported.root.blocks[0].source_anchor.clone();

    let mut invoke = imported_function;
    invoke.label = "Invoke imported Rust extern C function".to_owned();
    invoke.internal_graph_ref = None;
    invoke.implementation_ref = Some(INVOKE_IMPL.to_owned());
    invoke.ports = vec![
        port("flow_in", PortDirection::In, PortChannel::Flow, None),
        port(
            "value",
            PortDirection::In,
            PortChannel::Data,
            Some(RUST_FUNCTION_CONTRACT),
        ),
        port(
            "result",
            PortDirection::Out,
            PortChannel::Data,
            Some("c:abi:int32"),
        ),
    ];
    invoke.source_anchor = Some(function_anchor);

    graph.blocks.extend([input, build, invoke]);
    graph.connections.extend([
        connection(
            "flow:build-invoke",
            "block:build-rust",
            "flow_out",
            "rust:artifact:python-rust-cabi:file/function:algoram_checked_triple",
            "flow_in",
        ),
        connection(
            "data:python-rust",
            "block:python-input",
            "value",
            "rust:artifact:python-rust-cabi:file/function:algoram_checked_triple",
            "value",
        ),
    ]);

    graph.validate().expect("execution graph validates");
    graph
}

fn shared_library_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "libalgoram-python-rust-{label}-{}.so",
        std::process::id()
    ))
}

fn implementations(shared_library: &Path, value: i32) -> ImplementationRegistry {
    let root = repo_root();
    let rust_source = root.join("fixtures/python-rust-cabi/bridge.rs");
    let python_source = root.join("fixtures/python-rust-cabi/call.py");

    let mut implementations = ImplementationRegistry::new();
    implementations
        .register(
            BUILD_IMPL,
            ProcessAction::new(
                "rustc",
                [
                    "--edition=2021".to_owned(),
                    "--crate-type".to_owned(),
                    "cdylib".to_owned(),
                    rust_source.display().to_string(),
                    "-o".to_owned(),
                    shared_library.display().to_string(),
                ],
            ),
        )
        .unwrap();
    implementations
        .register(
            INVOKE_IMPL,
            ProcessAction::new(
                "python3",
                [
                    python_source.display().to_string(),
                    shared_library.display().to_string(),
                    value.to_string(),
                ],
            ),
        )
        .unwrap();

    implementations
}

#[test]
fn route_reuses_existing_python_c_abi_connector_without_direct_python_rust_edge() {
    let routes = route_registry();

    assert_eq!(routes.connectors().len(), 2);
    assert!(routes.connectors().iter().all(|connector| {
        !(connector.source.as_str().starts_with("python:")
            && connector.target.as_str().starts_with("rust:"))
    }));

    let first = routes
        .connectors()
        .iter()
        .find(|connector| connector.id == PYTHON_C_ABI_CONNECTOR_ID)
        .expect("accepted Python→C ABI connector is registered");
    assert_eq!(first.source.as_str(), "python:ctypes:c_int");
    assert_eq!(first.target.as_str(), "c:abi:int32");
    assert_eq!(first.implementation_ref, "fixture:python-c-ctypes");

    let resolution = routes
        .resolve(&RouteRequest::automatic(
            "python:ctypes:c_int",
            RUST_FUNCTION_CONTRACT,
        ))
        .unwrap();
    let route = resolution.route().expect("Python→Rust route resolves");

    assert_eq!(
        route.connector_ids(),
        vec![PYTHON_C_ABI_CONNECTOR_ID, RUST_C_ABI_CONNECTOR_ID]
    );

    let inspection = route.to_inspection_graph().unwrap();
    assert_eq!(inspection.blocks.len(), 2);
    assert_eq!(inspection.connections.len(), 1);
    assert_eq!(
        inspection.blocks[0].extensions["interop"]["connector_id"],
        json!(PYTHON_C_ABI_CONNECTOR_ID)
    );
    assert_eq!(
        inspection.blocks[1].extensions["interop"]["connector_id"],
        json!(RUST_C_ABI_CONNECTOR_ID)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn real_python_ctypes_to_rust_execution_preserves_route_and_source_anchor() {
    let imported = import_fixture();
    let graph = execution_graph(&imported);
    let routes = route_registry();
    let shared_library = shared_library_path("success");
    let _ = fs::remove_file(&shared_library);
    let implementations = implementations(&shared_library, 14);

    let plan = Planner::lower(&graph, &implementations, &routes).unwrap();
    assert_eq!(plan.steps.len(), 2);

    let invoke_step = plan
        .steps
        .iter()
        .find(|step| step.implementation_ref == INVOKE_IMPL)
        .expect("invoke step exists");

    assert_eq!(
        invoke_step.route_connector_ids,
        vec![
            PYTHON_C_ABI_CONNECTOR_ID.to_owned(),
            RUST_C_ABI_CONNECTOR_ID.to_owned()
        ]
    );
    assert_eq!(invoke_step.source_anchors.len(), 1);
    assert_eq!(
        invoke_step.source_anchors[0].artifact_id,
        "artifact:python-rust-cabi"
    );

    let trace = ProcessRuntime::execute(&plan);
    assert!(trace.succeeded());
    assert_eq!(trace.entries.len(), 2);

    let invoke = trace
        .entries
        .iter()
        .find(|entry| entry.step_id == invoke_step.id)
        .expect("invoke trace exists");
    assert_eq!(invoke.status, TraceStatus::Succeeded);
    assert_eq!(invoke.stdout.trim(), "42");
    assert_eq!(
        invoke.route_connector_ids,
        vec![
            PYTHON_C_ABI_CONNECTOR_ID.to_owned(),
            RUST_C_ABI_CONNECTOR_ID.to_owned()
        ]
    );
    assert_eq!(invoke.source_anchors, invoke_step.source_anchors);

    let _ = fs::remove_file(&shared_library);
}

#[cfg(target_os = "linux")]
#[test]
fn native_rust_failure_remains_anchored_to_rust_source() {
    let imported = import_fixture();
    let graph = execution_graph(&imported);
    let routes = route_registry();
    let shared_library = shared_library_path("failure");
    let _ = fs::remove_file(&shared_library);
    let implementations = implementations(&shared_library, -7);

    let plan = Planner::lower(&graph, &implementations, &routes).unwrap();
    let invoke_step = plan
        .steps
        .iter()
        .find(|step| step.implementation_ref == INVOKE_IMPL)
        .expect("invoke step exists")
        .clone();

    let trace = ProcessRuntime::execute(&plan);
    let failure = trace.failed_entry().expect("native failure is observed");

    assert_eq!(failure.step_id, invoke_step.id);
    assert_eq!(failure.status, TraceStatus::Failed);
    assert_eq!(failure.exit_code, Some(23));
    assert!(failure.stderr.contains("native failure"));
    assert_eq!(failure.source_anchors, invoke_step.source_anchors);
    assert_eq!(
        failure.route_connector_ids,
        vec![
            PYTHON_C_ABI_CONNECTOR_ID.to_owned(),
            RUST_C_ABI_CONNECTOR_ID.to_owned()
        ]
    );

    let _ = fs::remove_file(&shared_library);
}
