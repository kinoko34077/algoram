use algoram_core::{
    Block, Extensions, Graph, GraphError, Port, PortDirection, PortRef,
};
use algoram_interop::RouteRegistry;
use algoram_runtime::{
    ExecutionPlan, ExecutionTrace, ImplementationRegistry, Planner, PlannerError, ProcessRuntime,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundaryBinding {
    pub external_port_id: String,
    pub internal: PortRef,
}

impl BoundaryBinding {
    pub fn new(
        external_port_id: impl Into<String>,
        internal_block_id: impl Into<String>,
        internal_port_id: impl Into<String>,
    ) -> Self {
        Self {
            external_port_id: external_port_id.into(),
            internal: PortRef {
                block_id: internal_block_id.into(),
                port_id: internal_port_id.into(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositeDefinition {
    pub id: String,
    pub label: String,
    pub internal_graph: Graph,
    pub ports: Vec<Port>,
    pub boundary_bindings: Vec<BoundaryBinding>,
}

impl CompositeDefinition {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        internal_graph: Graph,
        ports: Vec<Port>,
        boundary_bindings: Vec<BoundaryBinding>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            internal_graph,
            ports,
            boundary_bindings,
        }
    }

    pub fn validate(&self) -> Result<(), CompositeError> {
        if self.id.trim().is_empty() {
            return Err(CompositeError::EmptyDefinitionId);
        }

        self.internal_graph
            .validate()
            .map_err(CompositeError::Graph)?;

        let mut external_ports = BTreeMap::<&str, &Port>::new();
        for port in &self.ports {
            if external_ports.insert(port.id.as_str(), port).is_some() {
                return Err(CompositeError::DuplicateExternalPort(port.id.clone()));
            }
        }

        let mut bound_external = BTreeSet::<&str>::new();
        let mut bound_internal = BTreeSet::<(&str, &str)>::new();

        for binding in &self.boundary_bindings {
            let external = external_ports
                .get(binding.external_port_id.as_str())
                .copied()
                .ok_or_else(|| CompositeError::UnknownExternalPort {
                    port_id: binding.external_port_id.clone(),
                })?;

            if !bound_external.insert(binding.external_port_id.as_str()) {
                return Err(CompositeError::AmbiguousExternalBinding {
                    port_id: binding.external_port_id.clone(),
                });
            }

            let internal_block = self
                .internal_graph
                .blocks
                .iter()
                .find(|block| block.id == binding.internal.block_id)
                .ok_or_else(|| CompositeError::MissingInternalBlock {
                    external_port_id: binding.external_port_id.clone(),
                    block_id: binding.internal.block_id.clone(),
                })?;

            let internal = internal_block
                .ports
                .iter()
                .find(|port| port.id == binding.internal.port_id)
                .ok_or_else(|| CompositeError::MissingInternalPort {
                    external_port_id: binding.external_port_id.clone(),
                    block_id: binding.internal.block_id.clone(),
                    port_id: binding.internal.port_id.clone(),
                })?;

            if !bound_internal.insert((
                binding.internal.block_id.as_str(),
                binding.internal.port_id.as_str(),
            )) {
                return Err(CompositeError::AmbiguousInternalBinding {
                    block_id: binding.internal.block_id.clone(),
                    port_id: binding.internal.port_id.clone(),
                });
            }

            let expected_internal_direction = match external.direction {
                PortDirection::In => PortDirection::Out,
                PortDirection::Out => PortDirection::In,
            };
            if internal.direction != expected_internal_direction {
                return Err(CompositeError::BoundaryDirectionMismatch {
                    external_port_id: external.id.clone(),
                    external_direction: external.direction.clone(),
                    internal_block_id: internal_block.id.clone(),
                    internal_port_id: internal.id.clone(),
                    internal_direction: internal.direction.clone(),
                });
            }

            if internal.channel != external.channel {
                return Err(CompositeError::BoundaryChannelMismatch {
                    external_port_id: external.id.clone(),
                    internal_block_id: internal_block.id.clone(),
                    internal_port_id: internal.id.clone(),
                });
            }

            if internal.contract != external.contract {
                return Err(CompositeError::BoundaryContractMismatch {
                    external_port_id: external.id.clone(),
                    internal_block_id: internal_block.id.clone(),
                    internal_port_id: internal.id.clone(),
                });
            }
        }

        for port in &self.ports {
            if !bound_external.contains(port.id.as_str()) {
                return Err(CompositeError::UnboundExternalPort {
                    port_id: port.id.clone(),
                });
            }
        }

        Ok(())
    }

    pub fn instantiate(
        &self,
        instance_id: impl Into<String>,
        label: impl Into<String>,
    ) -> Result<Block, CompositeError> {
        self.validate()?;

        let instance_id = instance_id.into();
        if instance_id.trim().is_empty() {
            return Err(CompositeError::EmptyInstanceId);
        }

        let mut extensions = Extensions::new();
        extensions.insert(
            "composite".to_owned(),
            json!({
                "definition_id": self.id,
                "internal_graph_id": self.internal_graph.id,
                "boundary_bindings": self.boundary_bindings
            }),
        );

        Ok(Block {
            id: instance_id,
            label: label.into(),
            ports: self.ports.clone(),
            internal_graph_ref: Some(self.internal_graph.id.clone()),
            implementation_ref: None,
            definition_ref: Some(self.id.clone()),
            source_anchor: None,
            extensions,
            diagnostics: Vec::new(),
        })
    }

    pub fn open_instance<'a>(&'a self, instance: &Block) -> Result<&'a Graph, CompositeError> {
        self.validate()?;
        self.validate_instance(instance)?;
        Ok(&self.internal_graph)
    }

    pub fn lower_instance(
        &self,
        instance: &Block,
        implementations: &ImplementationRegistry,
        routes: &RouteRegistry,
    ) -> Result<ExecutionPlan, CompositeError> {
        self.validate()?;
        self.validate_instance(instance)?;

        let mut plan =
            Planner::lower(&self.internal_graph, implementations, routes).map_err(CompositeError::Planner)?;

        for step in &mut plan.steps {
            if !step.origin_block_ids.iter().any(|id| id == &instance.id) {
                step.origin_block_ids.insert(0, instance.id.clone());
            }
            step.id = format!("composite:{}:{}", instance.id, step.id);
        }

        Ok(plan)
    }

    pub fn execute_instance(
        &self,
        instance: &Block,
        implementations: &ImplementationRegistry,
        routes: &RouteRegistry,
    ) -> Result<ExecutionTrace, CompositeError> {
        let plan = self.lower_instance(instance, implementations, routes)?;
        Ok(ProcessRuntime::execute(&plan))
    }

    fn validate_instance(&self, instance: &Block) -> Result<(), CompositeError> {
        if instance.definition_ref.as_deref() != Some(self.id.as_str()) {
            return Err(CompositeError::InstanceDefinitionMismatch {
                instance_id: instance.id.clone(),
                expected: self.id.clone(),
                found: instance.definition_ref.clone(),
            });
        }

        if instance.internal_graph_ref.as_deref() != Some(self.internal_graph.id.as_str()) {
            return Err(CompositeError::InstanceGraphMismatch {
                instance_id: instance.id.clone(),
                expected: self.internal_graph.id.clone(),
                found: instance.internal_graph_ref.clone(),
            });
        }

        if instance.ports != self.ports {
            return Err(CompositeError::InstanceInterfaceMismatch {
                instance_id: instance.id.clone(),
            });
        }

        Ok(())
    }
}

#[derive(Debug)]
pub enum CompositeError {
    Graph(GraphError),
    Planner(PlannerError),
    EmptyDefinitionId,
    EmptyInstanceId,
    DuplicateExternalPort(String),
    UnknownExternalPort {
        port_id: String,
    },
    UnboundExternalPort {
        port_id: String,
    },
    AmbiguousExternalBinding {
        port_id: String,
    },
    AmbiguousInternalBinding {
        block_id: String,
        port_id: String,
    },
    MissingInternalBlock {
        external_port_id: String,
        block_id: String,
    },
    MissingInternalPort {
        external_port_id: String,
        block_id: String,
        port_id: String,
    },
    BoundaryDirectionMismatch {
        external_port_id: String,
        external_direction: PortDirection,
        internal_block_id: String,
        internal_port_id: String,
        internal_direction: PortDirection,
    },
    BoundaryChannelMismatch {
        external_port_id: String,
        internal_block_id: String,
        internal_port_id: String,
    },
    BoundaryContractMismatch {
        external_port_id: String,
        internal_block_id: String,
        internal_port_id: String,
    },
    InstanceDefinitionMismatch {
        instance_id: String,
        expected: String,
        found: Option<String>,
    },
    InstanceGraphMismatch {
        instance_id: String,
        expected: String,
        found: Option<String>,
    },
    InstanceInterfaceMismatch {
        instance_id: String,
    },
}

impl fmt::Display for CompositeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Graph(error) => write!(f, "invalid internal graph: {error}"),
            Self::Planner(error) => write!(f, "failed to lower internal graph: {error}"),
            Self::EmptyDefinitionId => write!(f, "composite definition id must not be empty"),
            Self::EmptyInstanceId => write!(f, "composite instance id must not be empty"),
            Self::DuplicateExternalPort(id) => write!(f, "duplicate external port '{id}'"),
            Self::UnknownExternalPort { port_id } => {
                write!(f, "boundary binding references unknown external port '{port_id}'")
            }
            Self::UnboundExternalPort { port_id } => {
                write!(f, "external port '{port_id}' has no boundary binding")
            }
            Self::AmbiguousExternalBinding { port_id } => {
                write!(f, "external port '{port_id}' has more than one boundary binding")
            }
            Self::AmbiguousInternalBinding { block_id, port_id } => write!(
                f,
                "internal boundary port '{block_id}.{port_id}' is bound more than once"
            ),
            Self::MissingInternalBlock {
                external_port_id,
                block_id,
            } => write!(
                f,
                "external port '{external_port_id}' references missing internal block '{block_id}'"
            ),
            Self::MissingInternalPort {
                external_port_id,
                block_id,
                port_id,
            } => write!(
                f,
                "external port '{external_port_id}' references missing internal port '{block_id}.{port_id}'"
            ),
            Self::BoundaryDirectionMismatch {
                external_port_id,
                external_direction,
                internal_block_id,
                internal_port_id,
                internal_direction,
            } => write!(
                f,
                "external port '{external_port_id}' direction {external_direction:?} is incompatible with internal boundary '{internal_block_id}.{internal_port_id}' direction {internal_direction:?}"
            ),
            Self::BoundaryChannelMismatch {
                external_port_id,
                internal_block_id,
                internal_port_id,
            } => write!(
                f,
                "external port '{external_port_id}' channel does not match internal boundary '{internal_block_id}.{internal_port_id}'"
            ),
            Self::BoundaryContractMismatch {
                external_port_id,
                internal_block_id,
                internal_port_id,
            } => write!(
                f,
                "external port '{external_port_id}' contract does not match internal boundary '{internal_block_id}.{internal_port_id}'"
            ),
            Self::InstanceDefinitionMismatch {
                instance_id,
                expected,
                found,
            } => write!(
                f,
                "instance '{instance_id}' definition_ref mismatch: expected '{expected}', found {found:?}"
            ),
            Self::InstanceGraphMismatch {
                instance_id,
                expected,
                found,
            } => write!(
                f,
                "instance '{instance_id}' internal_graph_ref mismatch: expected '{expected}', found {found:?}"
            ),
            Self::InstanceInterfaceMismatch { instance_id } => {
                write!(f, "instance '{instance_id}' external interface differs from definition")
            }
        }
    }
}

impl Error for CompositeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Graph(error) => Some(error),
            Self::Planner(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use algoram_core::{
        Connection, PortChannel, SourceAnchor, SourceArtifact,
    };
    use algoram_interop::{Connector, ContractId, TransferMode};
    use algoram_runtime::{ProcessAction, TraceStatus};
    use serde_json::json;
    use std::fs;
    use std::path::{Path, PathBuf};

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

    fn basic_definition() -> CompositeDefinition {
        let mut graph = Graph::new("graph:composite:basic");

        let mut input = structural_block("boundary:input");
        input.ports.push(port(
            "value",
            PortDirection::Out,
            PortChannel::Data,
            Some("test:int"),
        ));

        let mut output = structural_block("boundary:output");
        output.ports.push(port(
            "result",
            PortDirection::In,
            PortChannel::Data,
            Some("test:int"),
        ));

        graph.blocks.extend([input, output]);
        graph.connections.push(data_connection(
            "data:boundary",
            "boundary:input",
            "value",
            "boundary:output",
            "result",
        ));

        CompositeDefinition::new(
            "definition:basic",
            "Basic",
            graph,
            vec![
                port(
                    "value",
                    PortDirection::In,
                    PortChannel::Data,
                    Some("test:int"),
                ),
                port(
                    "result",
                    PortDirection::Out,
                    PortChannel::Data,
                    Some("test:int"),
                ),
            ],
            vec![
                BoundaryBinding::new("value", "boundary:input", "value"),
                BoundaryBinding::new("result", "boundary:output", "result"),
            ],
        )
    }

    #[test]
    fn valid_definition_collapses_to_ordinary_block_and_reopens() {
        let definition = basic_definition();
        definition.validate().unwrap();

        let instance = definition
            .instantiate("block:basic-instance", "Basic instance")
            .unwrap();

        assert_eq!(
            instance.definition_ref.as_deref(),
            Some("definition:basic")
        );
        assert_eq!(
            instance.internal_graph_ref.as_deref(),
            Some("graph:composite:basic")
        );
        assert_eq!(instance.ports, definition.ports);

        let reopened = definition.open_instance(&instance).unwrap();
        assert_eq!(reopened, &definition.internal_graph);
    }

    #[test]
    fn missing_or_ambiguous_boundary_bindings_are_rejected() {
        let mut missing = basic_definition();
        missing.boundary_bindings.pop();
        assert!(matches!(
            missing.validate(),
            Err(CompositeError::UnboundExternalPort { .. })
        ));

        let mut ambiguous = basic_definition();
        ambiguous.boundary_bindings.push(BoundaryBinding::new(
            "value",
            "boundary:input",
            "value",
        ));
        assert!(matches!(
            ambiguous.validate(),
            Err(CompositeError::AmbiguousExternalBinding { .. })
        ));
    }

    #[test]
    fn contract_channel_and_direction_mismatches_are_rejected() {
        let mut direction = basic_definition();
        direction.internal_graph.blocks[0].ports[0].direction = PortDirection::In;
        assert!(matches!(
            direction.validate(),
            Err(CompositeError::BoundaryDirectionMismatch { .. })
        ));

        let mut channel = basic_definition();
        channel.internal_graph.blocks[0].ports[0].channel = PortChannel::Flow;
        assert!(matches!(
            channel.validate(),
            Err(CompositeError::BoundaryChannelMismatch { .. })
        ));

        let mut contract = basic_definition();
        contract.internal_graph.blocks[0].ports[0].contract = Some(json!("other:int"));
        assert!(matches!(
            contract.validate(),
            Err(CompositeError::BoundaryContractMismatch { .. })
        ));
    }

    fn parent_graph(id: &str, instance: Block) -> Graph {
        let mut graph = Graph::new(id);

        let mut source = structural_block(&format!("{id}:source"));
        source.ports.push(port(
            "out",
            PortDirection::Out,
            PortChannel::Data,
            Some("test:int"),
        ));

        let mut sink = structural_block(&format!("{id}:sink"));
        sink.ports.push(port(
            "in",
            PortDirection::In,
            PortChannel::Data,
            Some("test:int"),
        ));

        let instance_id = instance.id.clone();
        graph.blocks.extend([source, instance, sink]);
        graph.connections.extend([
            data_connection(
                &format!("{id}:source-instance"),
                &format!("{id}:source"),
                "out",
                &instance_id,
                "value",
            ),
            data_connection(
                &format!("{id}:instance-sink"),
                &instance_id,
                "result",
                &format!("{id}:sink"),
                "in",
            ),
        ]);
        graph
    }

    #[test]
    fn one_definition_can_be_reused_as_independent_instances() {
        let definition = basic_definition();
        let first = definition.instantiate("block:first", "First").unwrap();
        let second = definition.instantiate("block:second", "Second").unwrap();

        assert_ne!(first.id, second.id);
        assert_eq!(first.definition_ref, second.definition_ref);
        assert_eq!(first.internal_graph_ref, second.internal_graph_ref);
        assert_eq!(first.ports, second.ports);

        let graph_a = parent_graph("graph:parent-a", first);
        let graph_b = parent_graph("graph:parent-b", second);
        graph_a.validate().unwrap();
        graph_b.validate().unwrap();
    }

    fn whole_anchor(artifact_id: &str, source: &str) -> SourceAnchor {
        SourceAnchor {
            artifact_id: artifact_id.to_owned(),
            start_byte: 0,
            end_byte: source.len() as u64,
            start_line: Some(0),
            start_column: Some(0),
            end_line: None,
            end_column: None,
            semantic_key: None,
        }
    }

    fn function_anchor(artifact_id: &str, source: &str) -> SourceAnchor {
        let needle = "int32_t algoram_checked_double";
        let start = source.find(needle).expect("fixture function exists");
        SourceAnchor {
            artifact_id: artifact_id.to_owned(),
            start_byte: start as u64,
            end_byte: (start + needle.len()) as u64,
            start_line: None,
            start_column: None,
            end_line: None,
            end_column: None,
            semantic_key: Some("c:algoram_checked_double".to_owned()),
        }
    }

    fn heterogeneous_definition() -> CompositeDefinition {
        const C_SOURCE: &str = include_str!("../../fixtures/execution-plan/bridge.c");
        const PY_SOURCE: &str = include_str!("../../fixtures/execution-plan/call.py");

        let mut graph = Graph::new("graph:composite:python-c");
        graph.source_artifacts.extend([
            SourceArtifact {
                id: "artifact:composite-c".to_owned(),
                origin: "fixtures/execution-plan/bridge.c".to_owned(),
                language: "c".to_owned(),
                revision: None,
                content_hash: None,
                repository_origin: None,
                license: None,
            },
            SourceArtifact {
                id: "artifact:composite-python".to_owned(),
                origin: "fixtures/execution-plan/call.py".to_owned(),
                language: "python".to_owned(),
                revision: None,
                content_hash: None,
                repository_origin: None,
                license: None,
            },
        ]);

        let mut boundary_input = structural_block("block:boundary-input");
        boundary_input.ports.push(port(
            "value",
            PortDirection::Out,
            PortChannel::Data,
            Some("python:ctypes:c_int"),
        ));
        boundary_input.source_anchor =
            Some(whole_anchor("artifact:composite-python", PY_SOURCE));

        let mut build = structural_block("block:build-c");
        build.implementation_ref = Some("fixture:composite-build-c".to_owned());
        build.ports.push(port(
            "flow_out",
            PortDirection::Out,
            PortChannel::Flow,
            None,
        ));
        build.source_anchor = Some(whole_anchor("artifact:composite-c", C_SOURCE));

        let mut invoke = structural_block("block:invoke-c");
        invoke.implementation_ref = Some("fixture:composite-invoke-c".to_owned());
        invoke.ports.push(port(
            "flow_in",
            PortDirection::In,
            PortChannel::Flow,
            None,
        ));
        invoke.ports.push(port(
            "value",
            PortDirection::In,
            PortChannel::Data,
            Some("c:function:algoram_checked_double:int32"),
        ));
        invoke.ports.push(port(
            "result",
            PortDirection::Out,
            PortChannel::Data,
            Some("c:abi:int32"),
        ));
        invoke.source_anchor = Some(function_anchor("artifact:composite-c", C_SOURCE));

        let mut boundary_output = structural_block("block:boundary-output");
        boundary_output.ports.push(port(
            "result",
            PortDirection::In,
            PortChannel::Data,
            Some("c:abi:int32"),
        ));
        boundary_output.source_anchor =
            Some(whole_anchor("artifact:composite-python", PY_SOURCE));

        graph
            .blocks
            .extend([boundary_input, build, invoke, boundary_output]);
        graph.connections.extend([
            flow_connection("flow:build-invoke", "block:build-c", "block:invoke-c"),
            data_connection(
                "data:input-invoke",
                "block:boundary-input",
                "value",
                "block:invoke-c",
                "value",
            ),
            data_connection(
                "data:invoke-output",
                "block:invoke-c",
                "result",
                "block:boundary-output",
                "result",
            ),
        ]);

        CompositeDefinition::new(
            "definition:python-c-double",
            "Python/C checked double",
            graph,
            vec![
                port(
                    "value",
                    PortDirection::In,
                    PortChannel::Data,
                    Some("python:ctypes:c_int"),
                ),
                port(
                    "result",
                    PortDirection::Out,
                    PortChannel::Data,
                    Some("c:abi:int32"),
                ),
            ],
            vec![
                BoundaryBinding::new("value", "block:boundary-input", "value"),
                BoundaryBinding::new("result", "block:boundary-output", "result"),
            ],
        )
    }

    fn heterogeneous_routes() -> RouteRegistry {
        let mut routes = RouteRegistry::new();
        routes
            .register(
                Connector::new(
                    "python-ctypes-c-abi-int32",
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
                    "c-abi-call-algoram-checked-double",
                    ContractId::from("c:abi:int32"),
                    ContractId::from("c:function:algoram_checked_double:int32"),
                    "fixture:c:function:algoram_checked_double",
                )
                .with_transfer(TransferMode::Copy),
            )
            .unwrap();
        routes
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("composite crate has repository parent")
            .to_path_buf()
    }

    fn heterogeneous_implementations(
        shared_library: &Path,
        value: i32,
    ) -> ImplementationRegistry {
        let root = repo_root();
        let c_source = root.join("fixtures/execution-plan/bridge.c");
        let py_source = root.join("fixtures/execution-plan/call.py");

        let mut implementations = ImplementationRegistry::new();
        implementations
            .register(
                "fixture:composite-build-c",
                ProcessAction::new(
                    "cc",
                    [
                        "-std=c11".to_owned(),
                        "-Wall".to_owned(),
                        "-Wextra".to_owned(),
                        "-Werror".to_owned(),
                        "-fPIC".to_owned(),
                        "-shared".to_owned(),
                        c_source.display().to_string(),
                        "-o".to_owned(),
                        shared_library.display().to_string(),
                    ],
                ),
            )
            .unwrap();
        implementations
            .register(
                "fixture:composite-invoke-c",
                ProcessAction::new(
                    "python3",
                    [
                        py_source.display().to_string(),
                        shared_library.display().to_string(),
                        value.to_string(),
                    ],
                ),
            )
            .unwrap();
        implementations
    }

    #[test]
    fn heterogeneous_internal_route_survives_collapse_and_lifting() {
        let definition = heterogeneous_definition();
        definition.validate().unwrap();
        let instance = definition
            .instantiate("block:double", "Double through Python/C")
            .unwrap();

        let routes = heterogeneous_routes();

        let shared_library = std::env::temp_dir().join(format!(
            "libalgoram-composite-plan-{}.so",
            std::process::id()
        ));
        let implementations = heterogeneous_implementations(&shared_library, 21);

        let plan = definition
            .lower_instance(&instance, &implementations, &routes)
            .unwrap();

        assert_eq!(plan.reference_graph_id, definition.internal_graph.id);
        assert_eq!(plan.steps.len(), 2);
        assert!(plan.steps.iter().all(|step| {
            step.origin_block_ids.first().map(String::as_str) == Some("block:double")
        }));
        assert_eq!(
            plan.steps[1].origin_block_ids,
            vec!["block:double", "block:invoke-c"]
        );
        assert_eq!(
            plan.steps[1].route_connector_ids,
            vec![
                "python-ctypes-c-abi-int32",
                "c-abi-call-algoram-checked-double"
            ]
        );
        assert_eq!(
            plan.steps[1].source_anchors[0].artifact_id,
            "artifact:composite-c"
        );

        let _ = fs::remove_file(&shared_library);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn real_heterogeneous_fixture_executes_through_composite_and_traces_boundary() {
        let definition = heterogeneous_definition();
        let instance = definition
            .instantiate("block:double-runtime", "Double through Python/C")
            .unwrap();

        let shared_library = std::env::temp_dir().join(format!(
            "libalgoram-composite-success-{}.so",
            std::process::id()
        ));
        let _ = fs::remove_file(&shared_library);

        let implementations = heterogeneous_implementations(&shared_library, 21);
        let routes = heterogeneous_routes();
        let trace = definition
            .execute_instance(&instance, &implementations, &routes)
            .unwrap();

        assert!(trace.succeeded());
        assert_eq!(trace.entries.len(), 2);
        assert_eq!(trace.entries[0].status, TraceStatus::Succeeded);
        assert_eq!(trace.entries[1].status, TraceStatus::Succeeded);
        assert_eq!(trace.entries[1].stdout.trim(), "42");
        assert_eq!(
            trace.entries[1].origin_block_ids,
            vec!["block:double-runtime", "block:invoke-c"]
        );
        assert_eq!(
            trace.entries[1].route_connector_ids,
            vec![
                "python-ctypes-c-abi-int32",
                "c-abi-call-algoram-checked-double"
            ]
        );
        assert_eq!(
            trace.entries[1].source_anchors[0].artifact_id,
            "artifact:composite-c"
        );

        let _ = fs::remove_file(&shared_library);
    }
}
