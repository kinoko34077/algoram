use algoram_core::{Block, Graph, GraphError, Port, PortChannel, PortDirection, SourceAnchor};
use algoram_interop::{Resolution, RouteError, RouteRegistry, RouteRequest};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::process::Command;

pub const MAX_TRACE_TEXT_CHARS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessAction {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub argv_ports: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_dir: Option<String>,
}

impl ProcessAction {
    pub fn new(
        program: impl Into<String>,
        args: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
            argv_ports: Vec::new(),
            current_dir: None,
        }
    }

    pub fn with_argv_ports(
        mut self,
        port_ids: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.argv_ports = port_ids.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_current_dir(mut self, current_dir: impl Into<String>) -> Self {
        self.current_dir = Some(current_dir.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplementationChoice {
    pub candidates: Vec<String>,
    pub default_ref: String,
}

#[derive(Debug, Clone, Default)]
pub struct ImplementationProfileStore {
    observations_ns: BTreeMap<(String, String), u64>,
}

impl ImplementationProfileStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(
        &mut self,
        environment_key: impl Into<String>,
        implementation_ref: impl Into<String>,
        observed_ns: u64,
    ) {
        self.observations_ns.insert(
            (environment_key.into(), implementation_ref.into()),
            observed_ns,
        );
    }

    pub fn observed_ns(&self, environment_key: &str, implementation_ref: &str) -> Option<u64> {
        self.observations_ns
            .get(&(environment_key.to_owned(), implementation_ref.to_owned()))
            .copied()
    }
}

#[derive(Debug, Clone, Default)]
pub struct ImplementationRegistry {
    actions: BTreeMap<String, ProcessAction>,
    choices: BTreeMap<String, ImplementationChoice>,
}

impl ImplementationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        implementation_ref: impl Into<String>,
        action: ProcessAction,
    ) -> Result<(), PlannerError> {
        let implementation_ref = implementation_ref.into();
        if self.actions.contains_key(&implementation_ref)
            || self.choices.contains_key(&implementation_ref)
        {
            return Err(PlannerError::DuplicateImplementationRef(implementation_ref));
        }
        self.actions.insert(implementation_ref, action);
        Ok(())
    }

    pub fn register_choice(
        &mut self,
        logical_ref: impl Into<String>,
        candidates: impl IntoIterator<Item = impl Into<String>>,
        default_ref: impl Into<String>,
    ) -> Result<(), PlannerError> {
        let logical_ref = logical_ref.into();
        if self.actions.contains_key(&logical_ref) || self.choices.contains_key(&logical_ref) {
            return Err(PlannerError::DuplicateImplementationRef(logical_ref));
        }

        let candidates = candidates.into_iter().map(Into::into).collect::<Vec<_>>();
        if candidates.is_empty() {
            return Err(PlannerError::EmptyImplementationChoice { logical_ref });
        }

        let mut seen = BTreeSet::new();
        for candidate_ref in &candidates {
            if !seen.insert(candidate_ref.as_str()) {
                return Err(PlannerError::DuplicateImplementationCandidate {
                    logical_ref,
                    candidate_ref: candidate_ref.clone(),
                });
            }
            if !self.actions.contains_key(candidate_ref) {
                return Err(PlannerError::MissingImplementationCandidate {
                    logical_ref,
                    candidate_ref: candidate_ref.clone(),
                });
            }
        }

        let default_ref = default_ref.into();
        if !candidates.iter().any(|candidate| candidate == &default_ref) {
            return Err(PlannerError::DefaultImplementationNotCandidate {
                logical_ref,
                default_ref,
            });
        }

        self.choices.insert(
            logical_ref,
            ImplementationChoice {
                candidates,
                default_ref,
            },
        );
        Ok(())
    }

    pub fn action(&self, implementation_ref: &str) -> Option<&ProcessAction> {
        self.actions.get(implementation_ref)
    }

    pub fn choice(&self, logical_ref: &str) -> Option<&ImplementationChoice> {
        self.choices.get(logical_ref)
    }

    pub fn resolve(&self, implementation_ref: &str) -> Option<(&str, &ProcessAction)> {
        if let Some((concrete_ref, action)) = self.actions.get_key_value(implementation_ref) {
            return Some((concrete_ref.as_str(), action));
        }

        let choice = self.choices.get(implementation_ref)?;
        let (concrete_ref, action) = self.actions.get_key_value(&choice.default_ref)?;
        Some((concrete_ref.as_str(), action))
    }

    pub fn resolve_with_profiles(
        &self,
        implementation_ref: &str,
        profiles: &ImplementationProfileStore,
        environment_key: &str,
    ) -> Option<(&str, &ProcessAction)> {
        if let Some((concrete_ref, action)) = self.actions.get_key_value(implementation_ref) {
            return Some((concrete_ref.as_str(), action));
        }

        let choice = self.choices.get(implementation_ref)?;
        let selected_ref = choice
            .candidates
            .iter()
            .enumerate()
            .filter_map(|(index, candidate_ref)| {
                profiles
                    .observed_ns(environment_key, candidate_ref)
                    .map(|observed_ns| {
                        (
                            observed_ns,
                            candidate_ref != &choice.default_ref,
                            index,
                            candidate_ref,
                        )
                    })
            })
            .min_by_key(|(observed_ns, non_default, index, _)| (*observed_ns, *non_default, *index))
            .map(|(_, _, _, candidate_ref)| candidate_ref.as_str())
            .unwrap_or(choice.default_ref.as_str());

        let (concrete_ref, action) = self.actions.get_key_value(selected_ref)?;
        Some((concrete_ref.as_str(), action))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArgvBindingSource {
    ExternalPort { block_id: String, port_id: String },
    StepStdout { step_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArgvPortBinding {
    pub target_port_id: String,
    pub source: ArgvBindingSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionStep {
    pub id: String,
    pub implementation_ref: String,
    pub action: ProcessAction,
    pub origin_block_ids: Vec<String>,
    #[serde(default)]
    pub source_anchors: Vec<SourceAnchor>,
    #[serde(default)]
    pub route_connector_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub argv_bindings: Vec<ArgvPortBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub reference_graph_id: String,
    pub steps: Vec<ExecutionStep>,
}

pub struct Planner;

impl Planner {
    pub fn lower(
        graph: &Graph,
        implementations: &ImplementationRegistry,
        routes: &RouteRegistry,
    ) -> Result<ExecutionPlan, PlannerError> {
        Self::lower_internal(graph, implementations, routes, None)
    }

    pub fn lower_with_profiles(
        graph: &Graph,
        implementations: &ImplementationRegistry,
        routes: &RouteRegistry,
        profiles: &ImplementationProfileStore,
        environment_key: &str,
    ) -> Result<ExecutionPlan, PlannerError> {
        Self::lower_internal(
            graph,
            implementations,
            routes,
            Some((profiles, environment_key)),
        )
    }

    fn lower_internal(
        graph: &Graph,
        implementations: &ImplementationRegistry,
        routes: &RouteRegistry,
        profile_context: Option<(&ImplementationProfileStore, &str)>,
    ) -> Result<ExecutionPlan, PlannerError> {
        graph.validate().map_err(PlannerError::Graph)?;

        let order = stable_flow_order(graph)?;
        let mut steps = Vec::new();

        for block_index in order {
            let block = &graph.blocks[block_index];
            let Some(implementation_ref) = block.implementation_ref.as_deref() else {
                continue;
            };

            let resolved = match profile_context {
                Some((profiles, environment_key)) => implementations.resolve_with_profiles(
                    implementation_ref,
                    profiles,
                    environment_key,
                ),
                None => implementations.resolve(implementation_ref),
            };
            let (selected_implementation_ref, action) =
                resolved.ok_or_else(|| PlannerError::MissingImplementation {
                    block_id: block.id.clone(),
                    implementation_ref: implementation_ref.to_owned(),
                })?;
            let action = action.clone();

            let argv_bindings = derive_argv_bindings(graph, block, &action)?;
            let mut route_connector_ids = Vec::new();

            for connection in graph
                .connections
                .iter()
                .filter(|connection| connection.target.block_id == block.id)
            {
                let source_port = find_port(
                    graph,
                    &connection.source.block_id,
                    &connection.source.port_id,
                )
                .ok_or_else(|| PlannerError::MissingPort {
                    connection_id: connection.id.clone(),
                    block_id: connection.source.block_id.clone(),
                    port_id: connection.source.port_id.clone(),
                })?;
                let target_port = find_port(
                    graph,
                    &connection.target.block_id,
                    &connection.target.port_id,
                )
                .ok_or_else(|| PlannerError::MissingPort {
                    connection_id: connection.id.clone(),
                    block_id: connection.target.block_id.clone(),
                    port_id: connection.target.port_id.clone(),
                })?;

                if source_port.channel != PortChannel::Data {
                    continue;
                }

                let source_contract = string_contract(source_port);
                let target_contract = string_contract(target_port);

                match (source_contract, target_contract) {
                    (Some(source_contract), Some(target_contract))
                        if source_contract != target_contract =>
                    {
                        let resolution = routes
                            .resolve(&RouteRequest::automatic(source_contract, target_contract))
                            .map_err(PlannerError::Route)?;
                        match resolution {
                            Resolution::Resolved { route, .. } => {
                                route_connector_ids
                                    .extend(route.connector_ids().into_iter().map(str::to_owned));
                            }
                            Resolution::Unresolved { .. } => {
                                return Err(PlannerError::UnresolvedRoute {
                                    connection_id: connection.id.clone(),
                                    source_contract: source_contract.to_owned(),
                                    target_contract: target_contract.to_owned(),
                                });
                            }
                        }
                    }
                    (Some(_), Some(_)) | (None, None) => {}
                    _ => {
                        return Err(PlannerError::MissingInteropContract {
                            connection_id: connection.id.clone(),
                        });
                    }
                }
            }

            steps.push(ExecutionStep {
                id: format!("step:{}", block.id),
                implementation_ref: selected_implementation_ref.to_owned(),
                action,
                origin_block_ids: vec![block.id.clone()],
                source_anchors: block.source_anchor.clone().into_iter().collect(),
                route_connector_ids,
                argv_bindings,
            });
        }

        Ok(ExecutionPlan {
            reference_graph_id: graph.id.clone(),
            steps,
        })
    }
}

fn derive_argv_bindings(
    graph: &Graph,
    block: &Block,
    action: &ProcessAction,
) -> Result<Vec<ArgvPortBinding>, PlannerError> {
    let mut declared = BTreeSet::new();
    let mut bindings = Vec::with_capacity(action.argv_ports.len());

    for target_port_id in &action.argv_ports {
        if !declared.insert(target_port_id.as_str()) {
            return Err(PlannerError::DuplicateArgvPort {
                block_id: block.id.clone(),
                port_id: target_port_id.clone(),
            });
        }

        let target_port = block
            .ports
            .iter()
            .find(|port| port.id == *target_port_id)
            .ok_or_else(|| PlannerError::MissingArgvPort {
                block_id: block.id.clone(),
                port_id: target_port_id.clone(),
            })?;

        if target_port.direction != PortDirection::In || target_port.channel != PortChannel::Data {
            return Err(PlannerError::InvalidArgvPort {
                block_id: block.id.clone(),
                port_id: target_port_id.clone(),
            });
        }

        let incoming = graph
            .connections
            .iter()
            .filter(|connection| {
                connection.target.block_id == block.id
                    && connection.target.port_id == *target_port_id
            })
            .collect::<Vec<_>>();

        let connection = match incoming.as_slice() {
            [] => {
                return Err(PlannerError::MissingArgvBindingConnection {
                    block_id: block.id.clone(),
                    port_id: target_port_id.clone(),
                });
            }
            [connection] => *connection,
            _ => {
                return Err(PlannerError::AmbiguousArgvBindingConnection {
                    block_id: block.id.clone(),
                    port_id: target_port_id.clone(),
                    connection_ids: incoming
                        .iter()
                        .map(|connection| connection.id.clone())
                        .collect(),
                });
            }
        };

        let source_block = graph
            .blocks
            .iter()
            .find(|candidate| candidate.id == connection.source.block_id)
            .expect("validated graph contains argv binding source block");

        let source = if source_block.implementation_ref.is_some() {
            ArgvBindingSource::StepStdout {
                step_id: format!("step:{}", source_block.id),
            }
        } else {
            ArgvBindingSource::ExternalPort {
                block_id: connection.source.block_id.clone(),
                port_id: connection.source.port_id.clone(),
            }
        };

        bindings.push(ArgvPortBinding {
            target_port_id: target_port_id.clone(),
            source,
        });
    }

    Ok(bindings)
}

fn stable_flow_order(graph: &Graph) -> Result<Vec<usize>, PlannerError> {
    let mut block_indices = BTreeMap::<&str, usize>::new();
    for (index, block) in graph.blocks.iter().enumerate() {
        block_indices.insert(block.id.as_str(), index);
    }

    let mut indegree = vec![0usize; graph.blocks.len()];
    let mut outgoing = vec![Vec::<usize>::new(); graph.blocks.len()];

    for connection in &graph.connections {
        let Some(source_port) = find_port(
            graph,
            &connection.source.block_id,
            &connection.source.port_id,
        ) else {
            continue;
        };

        if source_port.channel != PortChannel::Flow {
            continue;
        }

        let source_index = *block_indices
            .get(connection.source.block_id.as_str())
            .expect("validated graph contains source block");
        let target_index = *block_indices
            .get(connection.target.block_id.as_str())
            .expect("validated graph contains target block");

        if !outgoing[source_index].contains(&target_index) {
            outgoing[source_index].push(target_index);
            indegree[target_index] += 1;
        }
    }

    for targets in &mut outgoing {
        targets.sort_unstable();
    }

    let mut ready = BTreeSet::<usize>::new();
    for (index, degree) in indegree.iter().enumerate() {
        if *degree == 0 {
            ready.insert(index);
        }
    }

    let mut order = Vec::with_capacity(graph.blocks.len());

    while let Some(index) = ready.iter().next().copied() {
        ready.remove(&index);
        order.push(index);

        for target in &outgoing[index] {
            indegree[*target] -= 1;
            if indegree[*target] == 0 {
                ready.insert(*target);
            }
        }
    }

    if order.len() != graph.blocks.len() {
        return Err(PlannerError::FlowCycle);
    }

    Ok(order)
}

fn find_port<'a>(graph: &'a Graph, block_id: &str, port_id: &str) -> Option<&'a Port> {
    graph
        .blocks
        .iter()
        .find(|block| block.id == block_id)?
        .ports
        .iter()
        .find(|port| port.id == port_id)
}

fn string_contract(port: &Port) -> Option<&str> {
    port.contract.as_ref().and_then(Value::as_str)
}

#[derive(Debug)]
pub enum PlannerError {
    Graph(GraphError),
    Route(RouteError),
    DuplicateImplementationRef(String),
    EmptyImplementationChoice {
        logical_ref: String,
    },
    DuplicateImplementationCandidate {
        logical_ref: String,
        candidate_ref: String,
    },
    MissingImplementationCandidate {
        logical_ref: String,
        candidate_ref: String,
    },
    DefaultImplementationNotCandidate {
        logical_ref: String,
        default_ref: String,
    },
    MissingImplementation {
        block_id: String,
        implementation_ref: String,
    },
    MissingPort {
        connection_id: String,
        block_id: String,
        port_id: String,
    },
    MissingInteropContract {
        connection_id: String,
    },
    UnresolvedRoute {
        connection_id: String,
        source_contract: String,
        target_contract: String,
    },
    DuplicateArgvPort {
        block_id: String,
        port_id: String,
    },
    MissingArgvPort {
        block_id: String,
        port_id: String,
    },
    InvalidArgvPort {
        block_id: String,
        port_id: String,
    },
    MissingArgvBindingConnection {
        block_id: String,
        port_id: String,
    },
    AmbiguousArgvBindingConnection {
        block_id: String,
        port_id: String,
        connection_ids: Vec<String>,
    },
    FlowCycle,
}

impl fmt::Display for PlannerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Graph(error) => write!(f, "invalid reference graph: {error}"),
            Self::Route(error) => write!(f, "route resolution failed: {error}"),
            Self::DuplicateImplementationRef(reference) => {
                write!(f, "duplicate implementation ref '{reference}'")
            }
            Self::EmptyImplementationChoice { logical_ref } => {
                write!(f, "implementation choice '{logical_ref}' has no candidates")
            }
            Self::DuplicateImplementationCandidate {
                logical_ref,
                candidate_ref,
            } => write!(
                f,
                "implementation choice '{logical_ref}' contains duplicate candidate '{candidate_ref}'"
            ),
            Self::MissingImplementationCandidate {
                logical_ref,
                candidate_ref,
            } => write!(
                f,
                "implementation choice '{logical_ref}' references unregistered candidate '{candidate_ref}'"
            ),
            Self::DefaultImplementationNotCandidate {
                logical_ref,
                default_ref,
            } => write!(
                f,
                "implementation choice '{logical_ref}' default '{default_ref}' is not one of its candidates"
            ),
            Self::MissingImplementation {
                block_id,
                implementation_ref,
            } => write!(
                f,
                "block '{block_id}' references missing execution implementation '{implementation_ref}'"
            ),
            Self::MissingPort {
                connection_id,
                block_id,
                port_id,
            } => write!(
                f,
                "connection '{connection_id}' references missing port '{block_id}.{port_id}'"
            ),
            Self::MissingInteropContract { connection_id } => write!(
                f,
                "data connection '{connection_id}' crosses a contract boundary with missing/non-string contract data"
            ),
            Self::UnresolvedRoute {
                connection_id,
                source_contract,
                target_contract,
            } => write!(
                f,
                "data connection '{connection_id}' has no known route from '{source_contract}' to '{target_contract}'"
            ),
            Self::DuplicateArgvPort { block_id, port_id } => write!(
                f,
                "block '{block_id}' declares argv Port '{port_id}' more than once"
            ),
            Self::MissingArgvPort { block_id, port_id } => write!(
                f,
                "block '{block_id}' declares missing argv Port '{port_id}'"
            ),
            Self::InvalidArgvPort { block_id, port_id } => write!(
                f,
                "block '{block_id}' argv Port '{port_id}' must be an input data Port"
            ),
            Self::MissingArgvBindingConnection { block_id, port_id } => write!(
                f,
                "block '{block_id}' argv Port '{port_id}' has no incoming data connection"
            ),
            Self::AmbiguousArgvBindingConnection {
                block_id,
                port_id,
                connection_ids,
            } => write!(
                f,
                "block '{block_id}' argv Port '{port_id}' has multiple incoming data connections: {}",
                connection_ids.join(", ")
            ),
            Self::FlowCycle => write!(f, "reference graph flow connections contain a cycle"),
        }
    }
}

impl Error for PlannerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Graph(error) => Some(error),
            Self::Route(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceStatus {
    Succeeded,
    Failed,
    NotRun,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceEntry {
    pub step_id: String,
    #[serde(default)]
    pub implementation_ref: String,
    pub status: TraceStatus,
    pub origin_block_ids: Vec<String>,
    #[serde(default)]
    pub source_anchors: Vec<SourceAnchor>,
    #[serde(default)]
    pub route_connector_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionTrace {
    pub reference_graph_id: String,
    pub entries: Vec<TraceEntry>,
}

impl ExecutionTrace {
    pub fn succeeded(&self) -> bool {
        self.entries
            .iter()
            .all(|entry| entry.status == TraceStatus::Succeeded)
    }

    pub fn failed_entry(&self) -> Option<&TraceEntry> {
        self.entries
            .iter()
            .find(|entry| entry.status == TraceStatus::Failed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeInput {
    pub source_block_id: String,
    pub source_port_id: String,
    pub value: String,
}

impl RuntimeInput {
    pub fn new(
        source_block_id: impl Into<String>,
        source_port_id: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        Self {
            source_block_id: source_block_id.into(),
            source_port_id: source_port_id.into(),
            value: value.into(),
        }
    }
}

pub struct ProcessRuntime;

impl ProcessRuntime {
    pub fn execute(plan: &ExecutionPlan) -> ExecutionTrace {
        Self::execute_with_inputs(plan, &[])
    }

    pub fn execute_with_inputs(plan: &ExecutionPlan, inputs: &[RuntimeInput]) -> ExecutionTrace {
        let mut entries = Vec::with_capacity(plan.steps.len());
        let mut successful_stdout = BTreeMap::<String, String>::new();
        let mut prior_failure = false;

        for step in &plan.steps {
            if prior_failure {
                entries.push(trace_entry(
                    step,
                    TraceStatus::NotRun,
                    None,
                    String::new(),
                    String::new(),
                ));
                continue;
            }

            let dynamic_args = match resolve_argv_bindings(step, inputs, &successful_stdout) {
                Ok(values) => values,
                Err(error) => {
                    prior_failure = true;
                    entries.push(trace_entry(
                        step,
                        TraceStatus::Failed,
                        None,
                        String::new(),
                        bounded_string(&error),
                    ));
                    continue;
                }
            };

            let mut command = Command::new(&step.action.program);
            command.args(&step.action.args);
            command.args(&dynamic_args);
            if let Some(current_dir) = &step.action.current_dir {
                command.current_dir(current_dir);
            }

            match command.output() {
                Ok(output) => {
                    let stdout = bounded_text(&output.stdout);
                    let stderr = bounded_text(&output.stderr);
                    let status = if output.status.success() {
                        successful_stdout.insert(step.id.clone(), stdout.clone());
                        TraceStatus::Succeeded
                    } else {
                        prior_failure = true;
                        TraceStatus::Failed
                    };
                    entries.push(trace_entry(
                        step,
                        status,
                        output.status.code(),
                        stdout,
                        stderr,
                    ));
                }
                Err(error) => {
                    prior_failure = true;
                    entries.push(trace_entry(
                        step,
                        TraceStatus::Failed,
                        None,
                        String::new(),
                        bounded_string(&error.to_string()),
                    ));
                }
            }
        }

        ExecutionTrace {
            reference_graph_id: plan.reference_graph_id.clone(),
            entries,
        }
    }
}

fn resolve_argv_bindings(
    step: &ExecutionStep,
    inputs: &[RuntimeInput],
    successful_stdout: &BTreeMap<String, String>,
) -> Result<Vec<String>, String> {
    let mut values = Vec::with_capacity(step.argv_bindings.len());

    for binding in &step.argv_bindings {
        let value = match &binding.source {
            ArgvBindingSource::ExternalPort { block_id, port_id } => inputs
                .iter()
                .find(|input| {
                    input.source_block_id == *block_id && input.source_port_id == *port_id
                })
                .map(|input| input.value.clone())
                .ok_or_else(|| {
                    format!(
                        "missing runtime input for external Port '{}.{}' required by step '{}' argv Port '{}'",
                        block_id, port_id, step.id, binding.target_port_id
                    )
                })?,
            ArgvBindingSource::StepStdout { step_id } => successful_stdout
                .get(step_id)
                .cloned()
                .ok_or_else(|| {
                    format!(
                        "missing successful stdout from step '{}' required by step '{}' argv Port '{}'",
                        step_id, step.id, binding.target_port_id
                    )
                })?,
        };
        values.push(value);
    }

    Ok(values)
}

fn trace_entry(
    step: &ExecutionStep,
    status: TraceStatus,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
) -> TraceEntry {
    TraceEntry {
        step_id: step.id.clone(),
        implementation_ref: step.implementation_ref.clone(),
        status,
        origin_block_ids: step.origin_block_ids.clone(),
        source_anchors: step.source_anchors.clone(),
        route_connector_ids: step.route_connector_ids.clone(),
        exit_code,
        stdout,
        stderr,
    }
}

fn bounded_text(bytes: &[u8]) -> String {
    bounded_string(&String::from_utf8_lossy(bytes))
}

fn bounded_string(value: &str) -> String {
    let count = value.chars().count();
    if count <= MAX_TRACE_TEXT_CHARS {
        return value.to_owned();
    }

    let mut result = value
        .chars()
        .take(MAX_TRACE_TEXT_CHARS.saturating_sub(16))
        .collect::<String>();
    result.push_str("\n...[truncated]");
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use algoram_core::{Block, Connection, Extensions, PortDirection, PortRef, SourceArtifact};
    use algoram_interop::{Connector, ContractId, TransferMode};
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

    fn flow_connection(id: &str, source: &str, target: &str) -> Connection {
        Connection {
            id: id.to_owned(),
            source: PortRef {
                block_id: source.to_owned(),
                port_id: "flow_out".to_owned(),
            },
            target: PortRef {
                block_id: target.to_owned(),
                port_id: "flow_in".to_owned(),
            },
            extensions: Extensions::new(),
        }
    }

    #[test]
    fn planner_preserves_unconstrained_reference_order() {
        let mut graph = Graph::new("graph:order");

        let mut second_named_first = block("block:z", Some("impl:z"));
        second_named_first
            .ports
            .push(port("flow_in", PortDirection::In, PortChannel::Flow, None));
        second_named_first.ports.push(port(
            "flow_out",
            PortDirection::Out,
            PortChannel::Flow,
            None,
        ));

        let mut first_named_second = block("block:a", Some("impl:a"));
        first_named_second
            .ports
            .push(port("flow_in", PortDirection::In, PortChannel::Flow, None));
        first_named_second.ports.push(port(
            "flow_out",
            PortDirection::Out,
            PortChannel::Flow,
            None,
        ));

        graph
            .blocks
            .extend([second_named_first, first_named_second]);

        let mut implementations = ImplementationRegistry::new();
        implementations
            .register(
                "impl:z",
                ProcessAction::new("z", std::iter::empty::<&str>()),
            )
            .unwrap();
        implementations
            .register(
                "impl:a",
                ProcessAction::new("a", std::iter::empty::<&str>()),
            )
            .unwrap();

        let plan = Planner::lower(&graph, &implementations, &RouteRegistry::new()).unwrap();

        assert_eq!(
            plan.steps
                .iter()
                .map(|step| step.origin_block_ids[0].as_str())
                .collect::<Vec<_>>(),
            vec!["block:z", "block:a"]
        );
    }

    #[test]
    fn explicit_flow_order_is_respected() {
        let mut graph = Graph::new("graph:flow-order");
        let mut later = block("block:later", Some("impl:later"));
        later
            .ports
            .push(port("flow_in", PortDirection::In, PortChannel::Flow, None));
        later.ports.push(port(
            "flow_out",
            PortDirection::Out,
            PortChannel::Flow,
            None,
        ));

        let mut earlier = block("block:earlier", Some("impl:earlier"));
        earlier
            .ports
            .push(port("flow_in", PortDirection::In, PortChannel::Flow, None));
        earlier.ports.push(port(
            "flow_out",
            PortDirection::Out,
            PortChannel::Flow,
            None,
        ));

        graph.blocks.extend([later, earlier]);
        graph.connections.push(flow_connection(
            "flow:earlier-later",
            "block:earlier",
            "block:later",
        ));

        let mut implementations = ImplementationRegistry::new();
        implementations
            .register(
                "impl:later",
                ProcessAction::new("later", std::iter::empty::<&str>()),
            )
            .unwrap();
        implementations
            .register(
                "impl:earlier",
                ProcessAction::new("earlier", std::iter::empty::<&str>()),
            )
            .unwrap();

        let plan = Planner::lower(&graph, &implementations, &RouteRegistry::new()).unwrap();

        assert_eq!(
            plan.steps
                .iter()
                .map(|step| step.origin_block_ids[0].as_str())
                .collect::<Vec<_>>(),
            vec!["block:earlier", "block:later"]
        );
    }

    #[test]
    fn planner_attaches_selected_interop_route() {
        let mut graph = Graph::new("graph:interop");
        let mut source = block("block:source", None);
        source.ports.push(port(
            "out",
            PortDirection::Out,
            PortChannel::Data,
            Some("python:ctypes:c_int"),
        ));
        let mut target = block("block:target", Some("impl:target"));
        target.ports.push(port(
            "in",
            PortDirection::In,
            PortChannel::Data,
            Some("c:function:target:int32"),
        ));
        graph.blocks.extend([source, target]);
        graph.connections.push(Connection {
            id: "data:source-target".to_owned(),
            source: PortRef {
                block_id: "block:source".to_owned(),
                port_id: "out".to_owned(),
            },
            target: PortRef {
                block_id: "block:target".to_owned(),
                port_id: "in".to_owned(),
            },
            extensions: Extensions::new(),
        });

        let mut implementations = ImplementationRegistry::new();
        implementations
            .register(
                "impl:target",
                ProcessAction::new("target", std::iter::empty::<&str>()),
            )
            .unwrap();

        let mut routes = RouteRegistry::new();
        routes
            .register(
                Connector::new(
                    "ctypes",
                    ContractId::from("python:ctypes:c_int"),
                    ContractId::from("c:abi:int32"),
                    "python:ctypes",
                )
                .with_transfer(TransferMode::Copy),
            )
            .unwrap();
        routes
            .register(
                Connector::new(
                    "c-abi",
                    ContractId::from("c:abi:int32"),
                    ContractId::from("c:function:target:int32"),
                    "c:abi",
                )
                .with_transfer(TransferMode::Copy),
            )
            .unwrap();

        let plan = Planner::lower(&graph, &implementations, &routes).unwrap();
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].route_connector_ids, vec!["ctypes", "c-abi"]);
    }

    #[test]
    fn unresolved_required_route_prevents_plan_creation() {
        let mut graph = Graph::new("graph:unresolved");
        let mut source = block("block:source", None);
        source.ports.push(port(
            "out",
            PortDirection::Out,
            PortChannel::Data,
            Some("domain:a"),
        ));
        let mut target = block("block:target", Some("impl:target"));
        target.ports.push(port(
            "in",
            PortDirection::In,
            PortChannel::Data,
            Some("domain:b"),
        ));
        graph.blocks.extend([source, target]);
        graph.connections.push(Connection {
            id: "data:unresolved".to_owned(),
            source: PortRef {
                block_id: "block:source".to_owned(),
                port_id: "out".to_owned(),
            },
            target: PortRef {
                block_id: "block:target".to_owned(),
                port_id: "in".to_owned(),
            },
            extensions: Extensions::new(),
        });

        let mut implementations = ImplementationRegistry::new();
        implementations
            .register(
                "impl:target",
                ProcessAction::new("target", std::iter::empty::<&str>()),
            )
            .unwrap();

        let error = Planner::lower(&graph, &implementations, &RouteRegistry::new()).unwrap_err();
        assert!(matches!(error, PlannerError::UnresolvedRoute { .. }));
    }

    #[test]
    fn planning_does_not_execute_actions() {
        let mut graph = Graph::new("graph:no-exec");
        graph.blocks.push(block("block:touch", Some("impl:touch")));

        let marker = std::env::temp_dir().join(format!(
            "algoram-planning-must-not-run-{}",
            std::process::id()
        ));
        let _ = fs::remove_file(&marker);

        let mut implementations = ImplementationRegistry::new();
        implementations
            .register(
                "impl:touch",
                ProcessAction::new(
                    "sh",
                    ["-c".to_owned(), format!("touch {}", marker.display())],
                ),
            )
            .unwrap();

        let _plan = Planner::lower(&graph, &implementations, &RouteRegistry::new()).unwrap();
        assert!(!marker.exists());
    }

    #[test]
    fn executor_marks_later_steps_not_run_after_failure() {
        let plan = ExecutionPlan {
            reference_graph_id: "graph:trace".to_owned(),
            steps: vec![
                ExecutionStep {
                    id: "step:fail".to_owned(),
                    implementation_ref: "impl:fail".to_owned(),
                    action: ProcessAction::new(
                        "definitely-not-an-algoram-command",
                        std::iter::empty::<&str>(),
                    ),
                    origin_block_ids: vec!["block:fail".to_owned()],
                    source_anchors: Vec::new(),
                    route_connector_ids: Vec::new(),
                    argv_bindings: Vec::new(),
                },
                ExecutionStep {
                    id: "step:later".to_owned(),
                    implementation_ref: "impl:later".to_owned(),
                    action: ProcessAction::new(
                        "definitely-not-run-after-failure",
                        std::iter::empty::<&str>(),
                    ),
                    origin_block_ids: vec!["block:later".to_owned()],
                    source_anchors: Vec::new(),
                    route_connector_ids: Vec::new(),
                    argv_bindings: Vec::new(),
                },
            ],
        };

        let trace = ProcessRuntime::execute(&plan);
        assert_eq!(trace.entries[0].status, TraceStatus::Failed);
        assert_eq!(trace.entries[1].status, TraceStatus::NotRun);
        assert!(trace.failed_entry().is_some());
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("runtime crate is inside repository root")
            .to_path_buf()
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

    fn fixture_graph() -> Graph {
        const C_SOURCE: &str = include_str!("../../fixtures/execution-plan/bridge.c");
        const PY_SOURCE: &str = include_str!("../../fixtures/execution-plan/call.py");

        let mut graph = Graph::new("graph:execution-fixture");
        graph.source_artifacts.extend([
            SourceArtifact {
                id: "artifact:execution-c".to_owned(),
                origin: "fixtures/execution-plan/bridge.c".to_owned(),
                language: "c".to_owned(),
                revision: None,
                content_hash: None,
                repository_origin: None,
                license: None,
            },
            SourceArtifact {
                id: "artifact:execution-python".to_owned(),
                origin: "fixtures/execution-plan/call.py".to_owned(),
                language: "python".to_owned(),
                revision: None,
                content_hash: None,
                repository_origin: None,
                license: None,
            },
        ]);

        let mut input = block("block:python-input", None);
        input.ports.push(port(
            "value",
            PortDirection::Out,
            PortChannel::Data,
            Some("python:ctypes:c_int"),
        ));
        input.source_anchor = Some(whole_anchor("artifact:execution-python", PY_SOURCE));

        let mut build = block("block:build-c", Some("fixture:build-c"));
        build.ports.push(port(
            "flow_out",
            PortDirection::Out,
            PortChannel::Flow,
            None,
        ));
        build.source_anchor = Some(whole_anchor("artifact:execution-c", C_SOURCE));

        let mut invoke = block("block:invoke-c", Some("fixture:invoke-c"));
        invoke
            .ports
            .push(port("flow_in", PortDirection::In, PortChannel::Flow, None));
        invoke.ports.push(port(
            "flow_out",
            PortDirection::Out,
            PortChannel::Flow,
            None,
        ));
        invoke.ports.push(port(
            "value",
            PortDirection::In,
            PortChannel::Data,
            Some("c:function:algoram_checked_double:int32"),
        ));
        invoke.source_anchor = Some(function_anchor("artifact:execution-c", C_SOURCE));

        let mut after = block("block:after", Some("fixture:after"));
        after
            .ports
            .push(port("flow_in", PortDirection::In, PortChannel::Flow, None));
        after.source_anchor = Some(whole_anchor("artifact:execution-python", PY_SOURCE));

        graph.blocks.extend([input, build, invoke, after]);
        graph.connections.extend([
            flow_connection("flow:build-invoke", "block:build-c", "block:invoke-c"),
            flow_connection("flow:invoke-after", "block:invoke-c", "block:after"),
            Connection {
                id: "data:input-invoke".to_owned(),
                source: PortRef {
                    block_id: "block:python-input".to_owned(),
                    port_id: "value".to_owned(),
                },
                target: PortRef {
                    block_id: "block:invoke-c".to_owned(),
                    port_id: "value".to_owned(),
                },
                extensions: Extensions::new(),
            },
        ]);
        graph
    }

    fn fixture_routes() -> RouteRegistry {
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

    fn fixture_implementations(shared_library: &Path, value: i32) -> ImplementationRegistry {
        let root = repo_root();
        let c_source = root.join("fixtures/execution-plan/bridge.c");
        let py_source = root.join("fixtures/execution-plan/call.py");

        let mut implementations = ImplementationRegistry::new();
        implementations
            .register(
                "fixture:build-c",
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
                "fixture:invoke-c",
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
            .register(
                "fixture:after",
                ProcessAction::new("python3", ["-c".to_owned(), "print('after')".to_owned()]),
            )
            .unwrap();
        implementations
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn real_fixture_success_executes_through_plan_and_records_route() {
        let shared_library = std::env::temp_dir().join(format!(
            "libalgoram-execution-success-{}.so",
            std::process::id()
        ));
        let _ = fs::remove_file(&shared_library);

        let graph = fixture_graph();
        let implementations = fixture_implementations(&shared_library, 21);
        let routes = fixture_routes();
        let plan = Planner::lower(&graph, &implementations, &routes).unwrap();

        assert_eq!(
            plan.steps
                .iter()
                .map(|step| step.origin_block_ids[0].as_str())
                .collect::<Vec<_>>(),
            vec!["block:build-c", "block:invoke-c", "block:after"]
        );
        assert_eq!(
            plan.steps[1].route_connector_ids,
            vec![
                "python-ctypes-c-abi-int32",
                "c-abi-call-algoram-checked-double"
            ]
        );

        let trace = ProcessRuntime::execute(&plan);
        assert!(trace.succeeded());
        assert!(trace.entries[1].stdout.trim() == "42");
        assert_eq!(
            trace.entries[1].source_anchors[0].artifact_id,
            "artifact:execution-c"
        );

        let _ = fs::remove_file(&shared_library);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn native_failure_maps_to_c_source_and_later_step_is_not_run() {
        let shared_library = std::env::temp_dir().join(format!(
            "libalgoram-execution-failure-{}.so",
            std::process::id()
        ));
        let _ = fs::remove_file(&shared_library);

        let graph = fixture_graph();
        let implementations = fixture_implementations(&shared_library, -2);
        let routes = fixture_routes();
        let plan = Planner::lower(&graph, &implementations, &routes).unwrap();
        let trace = ProcessRuntime::execute(&plan);

        assert!(!trace.succeeded());
        assert_eq!(trace.entries[0].status, TraceStatus::Succeeded);
        assert_eq!(trace.entries[1].status, TraceStatus::Failed);
        assert_eq!(trace.entries[1].exit_code, Some(23));
        assert!(trace.entries[1].stderr.contains("native failure"));
        assert_eq!(
            trace.entries[1].route_connector_ids,
            vec![
                "python-ctypes-c-abi-int32",
                "c-abi-call-algoram-checked-double"
            ]
        );
        assert_eq!(
            trace.entries[1].source_anchors[0].artifact_id,
            "artifact:execution-c"
        );
        assert_eq!(trace.entries[2].status, TraceStatus::NotRun);

        let _ = fs::remove_file(&shared_library);
    }
}
