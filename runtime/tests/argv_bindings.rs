use algoram_core::{
    Block, Connection, Extensions, Graph, Port, PortChannel, PortDirection, PortRef,
};
use algoram_interop::{Connector, ContractId, RouteRegistry, TransferMode};
use algoram_runtime::{
    ArgvBindingSource, ExecutionPlan, ImplementationRegistry, Planner, PlannerError, ProcessAction,
};

fn port(id: &str, direction: PortDirection, channel: PortChannel, contract: Option<&str>) -> Port {
    Port {
        id: id.to_owned(),
        direction,
        channel,
        contract: contract.map(|value| serde_json::json!(value)),
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

fn flow_connection(id: &str, source_block: &str, target_block: &str) -> Connection {
    Connection {
        id: id.to_owned(),
        source: PortRef {
            block_id: source_block.to_owned(),
            port_id: "flow_out".to_owned(),
        },
        target: PortRef {
            block_id: target_block.to_owned(),
            port_id: "flow_in".to_owned(),
        },
        extensions: Extensions::new(),
    }
}

fn binding_routes() -> RouteRegistry {
    let mut routes = RouteRegistry::new();
    routes
        .register(
            Connector::new(
                "external-to-process-text",
                ContractId::from("external:text"),
                ContractId::from("process:argv:text"),
                "fixture:text-route",
            )
            .with_transfer(TransferMode::Copy),
        )
        .unwrap();
    routes
}

fn binding_graph() -> Graph {
    let mut graph = Graph::new("graph:argv-bindings");

    let mut external = block("block:external", None);
    external.ports.push(port(
        "text",
        PortDirection::Out,
        PortChannel::Data,
        Some("external:text"),
    ));

    let mut producer = block("block:producer", Some("impl:producer"));
    producer.ports.push(port(
        "flow_out",
        PortDirection::Out,
        PortChannel::Flow,
        None,
    ));
    producer.ports.push(port(
        "stdout",
        PortDirection::Out,
        PortChannel::Data,
        Some("process:argv:text"),
    ));

    let mut target = block("block:target", Some("impl:target"));
    target
        .ports
        .push(port("flow_in", PortDirection::In, PortChannel::Flow, None));
    target.ports.push(port(
        "first",
        PortDirection::In,
        PortChannel::Data,
        Some("process:argv:text"),
    ));
    target.ports.push(port(
        "second",
        PortDirection::In,
        PortChannel::Data,
        Some("process:argv:text"),
    ));

    graph.blocks.extend([external, producer, target]);
    graph.connections.extend([
        flow_connection("flow:producer-target", "block:producer", "block:target"),
        data_connection(
            "data:external-first",
            "block:external",
            "text",
            "block:target",
            "first",
        ),
        data_connection(
            "data:producer-second",
            "block:producer",
            "stdout",
            "block:target",
            "second",
        ),
    ]);

    graph
}

fn binding_implementations() -> ImplementationRegistry {
    let mut implementations = ImplementationRegistry::new();
    implementations
        .register(
            "impl:producer",
            ProcessAction::new("producer", ["--producer-static"]),
        )
        .unwrap();
    implementations
        .register(
            "impl:target",
            ProcessAction::new("target", ["--fixed"]).with_argv_ports(["second", "first"]),
        )
        .unwrap();
    implementations
}

#[test]
fn planner_serializes_external_and_step_stdout_bindings_in_explicit_argv_order() {
    let graph = binding_graph();
    let implementations = binding_implementations();
    let plan = Planner::lower(&graph, &implementations, &binding_routes()).unwrap();

    assert_eq!(plan.steps.len(), 2);
    let target = &plan.steps[1];

    assert_eq!(target.origin_block_ids, vec!["block:target"]);
    assert_eq!(target.action.args, vec!["--fixed"]);
    assert_eq!(target.action.argv_ports, vec!["second", "first"]);
    assert_eq!(target.route_connector_ids, vec!["external-to-process-text"]);
    assert_eq!(target.argv_bindings.len(), 2);

    assert_eq!(target.argv_bindings[0].target_port_id, "second");
    assert_eq!(
        target.argv_bindings[0].source,
        ArgvBindingSource::StepStdout {
            step_id: "step:block:producer".to_owned(),
        }
    );

    assert_eq!(target.argv_bindings[1].target_port_id, "first");
    assert_eq!(
        target.argv_bindings[1].source,
        ArgvBindingSource::ExternalPort {
            block_id: "block:external".to_owned(),
            port_id: "text".to_owned(),
        }
    );

    let json = serde_json::to_string_pretty(&plan).unwrap();
    let restored: ExecutionPlan = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, plan);
}

#[test]
fn action_without_argv_ports_preserves_static_behavior_and_has_no_bindings() {
    let mut graph = Graph::new("graph:no-dynamic-argv");
    let mut source = block("block:source", None);
    source.ports.push(port(
        "out",
        PortDirection::Out,
        PortChannel::Data,
        Some("external:text"),
    ));
    let mut target = block("block:target", Some("impl:target"));
    target.ports.push(port(
        "in",
        PortDirection::In,
        PortChannel::Data,
        Some("process:argv:text"),
    ));
    graph.blocks.extend([source, target]);
    graph.connections.push(data_connection(
        "data:source-target",
        "block:source",
        "out",
        "block:target",
        "in",
    ));

    let mut implementations = ImplementationRegistry::new();
    implementations
        .register(
            "impl:target",
            ProcessAction::new("target", ["--fixed", "literal"]),
        )
        .unwrap();

    let plan = Planner::lower(&graph, &implementations, &binding_routes()).unwrap();
    assert_eq!(plan.steps.len(), 1);
    assert_eq!(plan.steps[0].action.args, vec!["--fixed", "literal"]);
    assert!(plan.steps[0].action.argv_ports.is_empty());
    assert!(plan.steps[0].argv_bindings.is_empty());
    assert_eq!(
        plan.steps[0].route_connector_ids,
        vec!["external-to-process-text"]
    );
}

fn planner_error_for_target(
    target_ports: Vec<Port>,
    connections: Vec<Connection>,
    argv_ports: &[&str],
) -> PlannerError {
    let mut graph = Graph::new("graph:invalid-argv-binding");

    let mut source_a = block("block:source-a", None);
    source_a.ports.push(port(
        "out",
        PortDirection::Out,
        PortChannel::Data,
        Some("process:argv:text"),
    ));

    let mut source_b = block("block:source-b", None);
    source_b.ports.push(port(
        "out",
        PortDirection::Out,
        PortChannel::Data,
        Some("process:argv:text"),
    ));

    let mut target = block("block:target", Some("impl:target"));
    target.ports = target_ports;

    graph.blocks.extend([source_a, source_b, target]);
    graph.connections = connections;

    let mut implementations = ImplementationRegistry::new();
    implementations
        .register(
            "impl:target",
            ProcessAction::new("target", ["--fixed"])
                .with_argv_ports(argv_ports.iter().copied()),
        )
        .unwrap();

    Planner::lower(&graph, &implementations, &RouteRegistry::new()).unwrap_err()
}

#[test]
fn planner_rejects_missing_declared_argv_port() {
    let error = planner_error_for_target(Vec::new(), Vec::new(), &["missing"]);
    assert!(matches!(
        error,
        PlannerError::MissingArgvPort {
            block_id,
            port_id
        } if block_id == "block:target" && port_id == "missing"
    ));
}

#[test]
fn planner_rejects_non_input_data_argv_port() {
    let target_ports = vec![port("value", PortDirection::In, PortChannel::Flow, None)];
    let error = planner_error_for_target(target_ports, Vec::new(), &["value"]);
    assert!(matches!(
        error,
        PlannerError::InvalidArgvPort {
            block_id,
            port_id
        } if block_id == "block:target" && port_id == "value"
    ));
}

#[test]
fn planner_rejects_argv_port_without_incoming_data_connection() {
    let target_ports = vec![port(
        "value",
        PortDirection::In,
        PortChannel::Data,
        Some("process:argv:text"),
    )];
    let error = planner_error_for_target(target_ports, Vec::new(), &["value"]);
    assert!(matches!(
        error,
        PlannerError::MissingArgvBindingConnection {
            block_id,
            port_id
        } if block_id == "block:target" && port_id == "value"
    ));
}

#[test]
fn planner_rejects_argv_port_with_multiple_incoming_data_connections() {
    let target_ports = vec![port(
        "value",
        PortDirection::In,
        PortChannel::Data,
        Some("process:argv:text"),
    )];
    let connections = vec![
        data_connection(
            "data:a-target",
            "block:source-a",
            "out",
            "block:target",
            "value",
        ),
        data_connection(
            "data:b-target",
            "block:source-b",
            "out",
            "block:target",
            "value",
        ),
    ];
    let error = planner_error_for_target(target_ports, connections, &["value"]);
    assert!(matches!(
        error,
        PlannerError::AmbiguousArgvBindingConnection {
            block_id,
            port_id,
            connection_ids
        } if block_id == "block:target"
            && port_id == "value"
            && connection_ids == vec!["data:a-target", "data:b-target"]
    ));
}

#[test]
fn planner_rejects_duplicate_argv_port_declaration() {
    let target_ports = vec![port(
        "value",
        PortDirection::In,
        PortChannel::Data,
        Some("process:argv:text"),
    )];
    let connections = vec![data_connection(
        "data:a-target",
        "block:source-a",
        "out",
        "block:target",
        "value",
    )];
    let error = planner_error_for_target(target_ports, connections, &["value", "value"]);
    assert!(matches!(
        error,
        PlannerError::DuplicateArgvPort {
            block_id,
            port_id
        } if block_id == "block:target" && port_id == "value"
    ));
}
