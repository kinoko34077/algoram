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

    pub fn with_choice_default(
        &self,
        logical_ref: &str,
        selected_ref: &str,
    ) -> Result<Self, PlannerError> {
        let mut selected = self.clone();
        let choice = selected.choices.get_mut(logical_ref).ok_or_else(|| {
            PlannerError::MissingImplementationChoice {
                logical_ref: logical_ref.to_owned(),
            }
        })?;

        if !choice
            .candidates
            .iter()
            .any(|candidate| candidate == selected_ref)
        {
            return Err(PlannerError::DefaultImplementationNotCandidate {
                logical_ref: logical_ref.to_owned(),
                default_ref: selected_ref.to_owned(),
            });
        }

        choice.default_ref = selected_ref.to_owned();
        Ok(selected)
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteOverride {
    pub connection_id: String,
    pub connector_ids: Vec<String>,
}

impl RouteOverride {
    pub fn new(
        connection_id: impl Into<String>,
        connector_ids: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            connection_id: connection_id.into(),
            connector_ids: connector_ids.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeLocationClass {
    LocalProcess,
    RuntimeAgent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeEndpoint {
    pub runtime_ref: String,
    pub class: RuntimeLocationClass,
    #[serde(default)]
    pub implementation_refs: Vec<String>,
}

impl RuntimeEndpoint {
    pub fn new(
        runtime_ref: impl Into<String>,
        class: RuntimeLocationClass,
        implementation_refs: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            runtime_ref: runtime_ref.into(),
            class,
            implementation_refs: implementation_refs.into_iter().map(Into::into).collect(),
        }
    }

    pub fn supports(&self, implementation_ref: &str) -> bool {
        self.implementation_refs
            .iter()
            .any(|candidate| candidate == implementation_ref)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepPlacement {
    pub step_id: String,
    pub runtime_ref: String,
}

impl StepPlacement {
    pub fn new(step_id: impl Into<String>, runtime_ref: impl Into<String>) -> Self {
        Self {
            step_id: step_id.into(),
            runtime_ref: runtime_ref.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacedExecutionPlan {
    pub plan: ExecutionPlan,
    pub placements: Vec<StepPlacement>,
}

impl PlacedExecutionPlan {
    pub fn new(plan: ExecutionPlan, placements: impl IntoIterator<Item = StepPlacement>) -> Self {
        Self {
            plan,
            placements: placements.into_iter().collect(),
        }
    }

    pub fn validate(&self, endpoints: &[RuntimeEndpoint]) -> Result<(), PlacementError> {
        let mut step_ids = BTreeSet::new();
        for step in &self.plan.steps {
            if !step_ids.insert(step.id.as_str()) {
                return Err(PlacementError::DuplicatePlanStepId(step.id.clone()));
            }
        }

        let mut endpoint_by_ref = BTreeMap::<&str, &RuntimeEndpoint>::new();
        for endpoint in endpoints {
            if endpoint.runtime_ref.trim().is_empty() {
                return Err(PlacementError::EmptyRuntimeRef);
            }
            if endpoint_by_ref
                .insert(endpoint.runtime_ref.as_str(), endpoint)
                .is_some()
            {
                return Err(PlacementError::DuplicateRuntimeRef(
                    endpoint.runtime_ref.clone(),
                ));
            }
        }

        let step_by_id = self
            .plan
            .steps
            .iter()
            .map(|step| (step.id.as_str(), step))
            .collect::<BTreeMap<_, _>>();
        let mut placed_step_ids = BTreeSet::new();

        for placement in &self.placements {
            let step = step_by_id.get(placement.step_id.as_str()).ok_or_else(|| {
                PlacementError::UnknownPlacementStep {
                    step_id: placement.step_id.clone(),
                }
            })?;

            if !placed_step_ids.insert(placement.step_id.as_str()) {
                return Err(PlacementError::DuplicatePlacement {
                    step_id: placement.step_id.clone(),
                });
            }

            let endpoint = endpoint_by_ref
                .get(placement.runtime_ref.as_str())
                .ok_or_else(|| PlacementError::UnknownRuntime {
                    step_id: placement.step_id.clone(),
                    runtime_ref: placement.runtime_ref.clone(),
                })?;

            if !endpoint.supports(&step.implementation_ref) {
                return Err(PlacementError::ImplementationUnavailable {
                    step_id: step.id.clone(),
                    runtime_ref: endpoint.runtime_ref.clone(),
                    implementation_ref: step.implementation_ref.clone(),
                });
            }
        }

        for step in &self.plan.steps {
            if !placed_step_ids.contains(step.id.as_str()) {
                return Err(PlacementError::MissingPlacement {
                    step_id: step.id.clone(),
                });
            }
        }

        Ok(())
    }

    pub fn runtime_for_step(&self, step_id: &str) -> Option<&str> {
        self.placements
            .iter()
            .find(|placement| placement.step_id == step_id)
            .map(|placement| placement.runtime_ref.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlacementError {
    DuplicatePlanStepId(String),
    EmptyRuntimeRef,
    DuplicateRuntimeRef(String),
    UnknownPlacementStep {
        step_id: String,
    },
    DuplicatePlacement {
        step_id: String,
    },
    MissingPlacement {
        step_id: String,
    },
    UnknownRuntime {
        step_id: String,
        runtime_ref: String,
    },
    ImplementationUnavailable {
        step_id: String,
        runtime_ref: String,
        implementation_ref: String,
    },
}

impl fmt::Display for PlacementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicatePlanStepId(step_id) => {
                write!(f, "execution Plan contains duplicate step id '{step_id}'")
            }
            Self::EmptyRuntimeRef => write!(f, "runtime endpoint ref must not be empty"),
            Self::DuplicateRuntimeRef(runtime_ref) => {
                write!(f, "duplicate runtime endpoint ref '{runtime_ref}'")
            }
            Self::UnknownPlacementStep { step_id } => {
                write!(f, "placement references unknown execution step '{step_id}'")
            }
            Self::DuplicatePlacement { step_id } => {
                write!(f, "execution step '{step_id}' is placed more than once")
            }
            Self::MissingPlacement { step_id } => {
                write!(f, "execution step '{step_id}' has no runtime placement")
            }
            Self::UnknownRuntime {
                step_id,
                runtime_ref,
            } => write!(
                f,
                "execution step '{step_id}' references unknown runtime '{runtime_ref}'"
            ),
            Self::ImplementationUnavailable {
                step_id,
                runtime_ref,
                implementation_ref,
            } => write!(
                f,
                "execution step '{step_id}' implementation '{implementation_ref}' is unavailable at runtime '{runtime_ref}'"
            ),
        }
    }
}

impl Error for PlacementError {}

pub struct Planner;

impl Planner {
    pub fn lower(
        graph: &Graph,
        implementations: &ImplementationRegistry,
        routes: &RouteRegistry,
    ) -> Result<ExecutionPlan, PlannerError> {
        Self::lower_internal(graph, implementations, routes, None, &[])
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
            &[],
        )
    }

    pub fn lower_with_route_overrides(
        graph: &Graph,
        implementations: &ImplementationRegistry,
        routes: &RouteRegistry,
        route_overrides: &[RouteOverride],
    ) -> Result<ExecutionPlan, PlannerError> {
        Self::lower_internal(graph, implementations, routes, None, route_overrides)
    }

    fn lower_internal(
        graph: &Graph,
        implementations: &ImplementationRegistry,
        routes: &RouteRegistry,
        profile_context: Option<(&ImplementationProfileStore, &str)>,
        route_overrides: &[RouteOverride],
    ) -> Result<ExecutionPlan, PlannerError> {
        graph.validate().map_err(PlannerError::Graph)?;

        let graph_connection_ids = graph
            .connections
            .iter()
            .map(|connection| connection.id.as_str())
            .collect::<BTreeSet<_>>();
        let mut override_by_connection = BTreeMap::<&str, &RouteOverride>::new();
        for route_override in route_overrides {
            if route_override.connection_id.trim().is_empty() {
                return Err(PlannerError::EmptyRouteOverrideConnectionId);
            }
            if !graph_connection_ids.contains(route_override.connection_id.as_str()) {
                return Err(PlannerError::UnknownRouteOverrideConnection {
                    connection_id: route_override.connection_id.clone(),
                });
            }
            if override_by_connection
                .insert(route_override.connection_id.as_str(), route_override)
                .is_some()
            {
                return Err(PlannerError::DuplicateRouteOverride {
                    connection_id: route_override.connection_id.clone(),
                });
            }
        }
        let mut applied_route_overrides = BTreeSet::<String>::new();

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
                        let request = match override_by_connection.get(connection.id.as_str()) {
                            Some(route_override) => {
                                applied_route_overrides.insert(connection.id.clone());
                                RouteRequest::explicit(
                                    source_contract,
                                    target_contract,
                                    route_override.connector_ids.iter().cloned(),
                                )
                            }
                            None => RouteRequest::automatic(source_contract, target_contract),
                        };
                        let resolution = routes.resolve(&request).map_err(PlannerError::Route)?;
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

        for connection_id in override_by_connection.keys() {
            if !applied_route_overrides.contains(*connection_id) {
                return Err(PlannerError::RouteOverrideNotApplicable {
                    connection_id: (*connection_id).to_owned(),
                });
            }
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
    MissingImplementationChoice {
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
    EmptyRouteOverrideConnectionId,
    UnknownRouteOverrideConnection {
        connection_id: String,
    },
    DuplicateRouteOverride {
        connection_id: String,
    },
    RouteOverrideNotApplicable {
        connection_id: String,
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
            Self::MissingImplementationChoice { logical_ref } => {
                write!(f, "implementation choice '{logical_ref}' is not registered")
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
            Self::EmptyRouteOverrideConnectionId => {
                write!(f, "route override connection_id must not be empty")
            }
            Self::UnknownRouteOverrideConnection { connection_id } => write!(
                f,
                "route override references unknown connection '{connection_id}'"
            ),
            Self::DuplicateRouteOverride { connection_id } => write!(
                f,
                "route override for connection '{connection_id}' is declared more than once"
            ),
            Self::RouteOverrideNotApplicable { connection_id } => write!(
                f,
                "route override for connection '{connection_id}' is not applicable to a cross-contract executable data connection"
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeAgentRequest {
    pub plan: ExecutionPlan,
    #[serde(default)]
    pub inputs: Vec<RuntimeInput>,
}

impl RuntimeAgentRequest {
    pub fn new(plan: ExecutionPlan, inputs: impl IntoIterator<Item = RuntimeInput>) -> Self {
        Self {
            plan,
            inputs: inputs.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RuntimeAgentResponse {
    Executed { trace: ExecutionTrace },
    Rejected { error: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionAccessClass {
    AmbientHostProcess,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionAccessRequirement {
    pub step_id: String,
    pub implementation_ref: String,
    pub origin_block_ids: Vec<String>,
    pub access_class: ExecutionAccessClass,
    pub action: ProcessAction,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub argv_bindings: Vec<ArgvPortBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionAccessReport {
    pub reference_graph_id: String,
    pub requirements: Vec<ExecutionAccessRequirement>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecutionPolicy {
    allowed_implementation_refs: BTreeSet<String>,
}

impl ExecutionPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allow(&mut self, implementation_ref: impl Into<String>) {
        self.allowed_implementation_refs
            .insert(implementation_ref.into());
    }

    pub fn allows(&self, implementation_ref: &str) -> bool {
        self.allowed_implementation_refs
            .contains(implementation_ref)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionSecurityError {
    MissingTrustedImplementation {
        step_id: String,
        implementation_ref: String,
    },
    TrustedActionMismatch {
        step_id: String,
        implementation_ref: String,
    },
    ArgvBindingShapeMismatch {
        step_id: String,
        implementation_ref: String,
        expected_ports: Vec<String>,
        found_ports: Vec<String>,
    },
    DeniedImplementation {
        step_id: String,
        implementation_ref: String,
    },
}

impl fmt::Display for ExecutionSecurityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTrustedImplementation {
                step_id,
                implementation_ref,
            } => write!(
                f,
                "step '{step_id}' references implementation '{implementation_ref}' that is not a trusted local concrete action"
            ),
            Self::TrustedActionMismatch {
                step_id,
                implementation_ref,
            } => write!(
                f,
                "step '{step_id}' action does not match trusted local implementation '{implementation_ref}'"
            ),
            Self::ArgvBindingShapeMismatch {
                step_id,
                implementation_ref,
                expected_ports,
                found_ports,
            } => write!(
                f,
                "step '{step_id}' argv binding shape for implementation '{implementation_ref}' does not match trusted argv_ports: expected [{}], found [{}]",
                expected_ports.join(", "),
                found_ports.join(", ")
            ),
            Self::DeniedImplementation {
                step_id,
                implementation_ref,
            } => write!(
                f,
                "step '{step_id}' implementation '{implementation_ref}' is not allowed by execution policy"
            ),
        }
    }
}

impl Error for ExecutionSecurityError {}

pub struct GuardedProcessRuntime;

impl GuardedProcessRuntime {
    pub fn inspect(
        plan: &ExecutionPlan,
        trusted_implementations: &ImplementationRegistry,
    ) -> Result<ExecutionAccessReport, ExecutionSecurityError> {
        let mut requirements = Vec::with_capacity(plan.steps.len());

        for step in &plan.steps {
            let trusted_action = trusted_implementations
                .action(&step.implementation_ref)
                .ok_or_else(|| ExecutionSecurityError::MissingTrustedImplementation {
                    step_id: step.id.clone(),
                    implementation_ref: step.implementation_ref.clone(),
                })?;

            if trusted_action != &step.action {
                return Err(ExecutionSecurityError::TrustedActionMismatch {
                    step_id: step.id.clone(),
                    implementation_ref: step.implementation_ref.clone(),
                });
            }

            let found_ports = step
                .argv_bindings
                .iter()
                .map(|binding| binding.target_port_id.clone())
                .collect::<Vec<_>>();
            if found_ports != trusted_action.argv_ports {
                return Err(ExecutionSecurityError::ArgvBindingShapeMismatch {
                    step_id: step.id.clone(),
                    implementation_ref: step.implementation_ref.clone(),
                    expected_ports: trusted_action.argv_ports.clone(),
                    found_ports,
                });
            }

            requirements.push(ExecutionAccessRequirement {
                step_id: step.id.clone(),
                implementation_ref: step.implementation_ref.clone(),
                origin_block_ids: step.origin_block_ids.clone(),
                access_class: ExecutionAccessClass::AmbientHostProcess,
                action: step.action.clone(),
                argv_bindings: step.argv_bindings.clone(),
            });
        }

        Ok(ExecutionAccessReport {
            reference_graph_id: plan.reference_graph_id.clone(),
            requirements,
        })
    }

    pub fn preflight(
        plan: &ExecutionPlan,
        trusted_implementations: &ImplementationRegistry,
        policy: &ExecutionPolicy,
    ) -> Result<ExecutionAccessReport, ExecutionSecurityError> {
        let report = Self::inspect(plan, trusted_implementations)?;

        for requirement in &report.requirements {
            if !policy.allows(&requirement.implementation_ref) {
                return Err(ExecutionSecurityError::DeniedImplementation {
                    step_id: requirement.step_id.clone(),
                    implementation_ref: requirement.implementation_ref.clone(),
                });
            }
        }

        Ok(report)
    }

    pub fn execute(
        plan: &ExecutionPlan,
        trusted_implementations: &ImplementationRegistry,
        policy: &ExecutionPolicy,
    ) -> Result<ExecutionTrace, ExecutionSecurityError> {
        Self::execute_with_inputs(plan, &[], trusted_implementations, policy)
    }

    pub fn execute_with_inputs(
        plan: &ExecutionPlan,
        inputs: &[RuntimeInput],
        trusted_implementations: &ImplementationRegistry,
        policy: &ExecutionPolicy,
    ) -> Result<ExecutionTrace, ExecutionSecurityError> {
        Self::preflight(plan, trusted_implementations, policy)?;
        Ok(ProcessRuntime::execute_with_inputs(plan, inputs))
    }
}

pub struct RuntimeAgent;

impl RuntimeAgent {
    pub fn handle(
        request: RuntimeAgentRequest,
        trusted_implementations: &ImplementationRegistry,
        policy: &ExecutionPolicy,
    ) -> RuntimeAgentResponse {
        match GuardedProcessRuntime::execute_with_inputs(
            &request.plan,
            &request.inputs,
            trusted_implementations,
            policy,
        ) {
            Ok(trace) => RuntimeAgentResponse::Executed { trace },
            Err(error) => RuntimeAgentResponse::Rejected {
                error: error.to_string(),
            },
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedTraceEntry {
    pub runtime_ref: String,
    pub entry: TraceEntry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedTransfer {
    pub source_step_id: String,
    pub target_step_id: String,
    pub target_port_id: String,
    pub source_runtime_ref: String,
    pub target_runtime_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedExecutionTrace {
    pub reference_graph_id: String,
    pub entries: Vec<DistributedTraceEntry>,
    #[serde(default)]
    pub transfers: Vec<DistributedTransfer>,
}

impl DistributedExecutionTrace {
    pub fn succeeded(&self) -> bool {
        self.entries
            .iter()
            .all(|entry| entry.entry.status == TraceStatus::Succeeded)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DistributedExecutionError {
    Placement(PlacementError),
    MissingTransferredOutput {
        source_step_id: String,
        target_step_id: String,
    },
    LocalRejected {
        runtime_ref: String,
        error: String,
    },
    AgentUnavailable {
        runtime_ref: String,
        error: String,
    },
    AgentRejected {
        runtime_ref: String,
        error: String,
    },
    MalformedAgentResponse {
        runtime_ref: String,
        detail: String,
    },
}

impl fmt::Display for DistributedExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Placement(error) => write!(f, "invalid distributed placement: {error}"),
            Self::MissingTransferredOutput {
                source_step_id,
                target_step_id,
            } => write!(
                f,
                "distributed step '{target_step_id}' requires unavailable stdout from '{source_step_id}'"
            ),
            Self::LocalRejected { runtime_ref, error } => {
                write!(f, "local runtime '{runtime_ref}' rejected execution: {error}")
            }
            Self::AgentUnavailable { runtime_ref, error } => {
                write!(f, "runtime agent '{runtime_ref}' is unavailable: {error}")
            }
            Self::AgentRejected { runtime_ref, error } => {
                write!(f, "runtime agent '{runtime_ref}' rejected execution: {error}")
            }
            Self::MalformedAgentResponse {
                runtime_ref,
                detail,
            } => write!(
                f,
                "runtime agent '{runtime_ref}' returned malformed execution trace: {detail}"
            ),
        }
    }
}

impl Error for DistributedExecutionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Placement(error) => Some(error),
            _ => None,
        }
    }
}

pub struct DistributedRuntime;

impl DistributedRuntime {
    pub fn execute_with_agent<F>(
        placed: &PlacedExecutionPlan,
        endpoints: &[RuntimeEndpoint],
        local_implementations: &ImplementationRegistry,
        local_policy: &ExecutionPolicy,
        external_inputs: &[RuntimeInput],
        mut dispatch_agent: F,
    ) -> Result<DistributedExecutionTrace, DistributedExecutionError>
    where
        F: FnMut(&str, RuntimeAgentRequest) -> Result<RuntimeAgentResponse, String>,
    {
        placed
            .validate(endpoints)
            .map_err(DistributedExecutionError::Placement)?;

        let mut distributed_entries = Vec::with_capacity(placed.plan.steps.len());
        let mut transfers = Vec::new();
        let mut completed_stdout = BTreeMap::<String, String>::new();
        let mut completed_runtime = BTreeMap::<String, String>::new();

        let mut start = 0usize;
        while start < placed.plan.steps.len() {
            let runtime_ref = placed
                .runtime_for_step(&placed.plan.steps[start].id)
                .expect("validated placed Plan has runtime for every step")
                .to_owned();

            let mut end = start + 1;
            while end < placed.plan.steps.len()
                && placed.runtime_for_step(&placed.plan.steps[end].id) == Some(runtime_ref.as_str())
            {
                end += 1;
            }

            let endpoint = endpoints
                .iter()
                .find(|endpoint| endpoint.runtime_ref == runtime_ref)
                .expect("validated placed Plan references known endpoint");

            let segment_step_ids = placed.plan.steps[start..end]
                .iter()
                .map(|step| step.id.clone())
                .collect::<BTreeSet<_>>();
            let mut segment_steps = placed.plan.steps[start..end].to_vec();
            let mut segment_inputs = external_inputs.to_vec();

            for step in &mut segment_steps {
                for binding in &mut step.argv_bindings {
                    let source_step_id = match &binding.source {
                        ArgvBindingSource::StepStdout { step_id }
                            if !segment_step_ids.contains(step_id) =>
                        {
                            Some(step_id.clone())
                        }
                        _ => None,
                    };

                    let Some(source_step_id) = source_step_id else {
                        continue;
                    };

                    let value =
                        completed_stdout
                            .get(&source_step_id)
                            .cloned()
                            .ok_or_else(|| DistributedExecutionError::MissingTransferredOutput {
                                source_step_id: source_step_id.clone(),
                                target_step_id: step.id.clone(),
                            })?;
                    let source_runtime_ref = completed_runtime
                        .get(&source_step_id)
                        .cloned()
                        .ok_or_else(|| DistributedExecutionError::MissingTransferredOutput {
                            source_step_id: source_step_id.clone(),
                            target_step_id: step.id.clone(),
                        })?;

                    let transfer_block_id = format!("distributed:stdout:{source_step_id}");
                    let transfer_port_id = binding.target_port_id.clone();
                    binding.source = ArgvBindingSource::ExternalPort {
                        block_id: transfer_block_id.clone(),
                        port_id: transfer_port_id.clone(),
                    };
                    segment_inputs.push(RuntimeInput::new(
                        transfer_block_id,
                        transfer_port_id.clone(),
                        value,
                    ));

                    if source_runtime_ref != runtime_ref {
                        transfers.push(DistributedTransfer {
                            source_step_id,
                            target_step_id: step.id.clone(),
                            target_port_id: transfer_port_id,
                            source_runtime_ref,
                            target_runtime_ref: runtime_ref.clone(),
                        });
                    }
                }
            }

            let segment_plan = ExecutionPlan {
                reference_graph_id: placed.plan.reference_graph_id.clone(),
                steps: segment_steps,
            };

            let trace = match endpoint.class {
                RuntimeLocationClass::LocalProcess => GuardedProcessRuntime::execute_with_inputs(
                    &segment_plan,
                    &segment_inputs,
                    local_implementations,
                    local_policy,
                )
                .map_err(|error| DistributedExecutionError::LocalRejected {
                    runtime_ref: runtime_ref.clone(),
                    error: error.to_string(),
                })?,
                RuntimeLocationClass::RuntimeAgent => {
                    let response = dispatch_agent(
                        &runtime_ref,
                        RuntimeAgentRequest::new(segment_plan.clone(), segment_inputs),
                    )
                    .map_err(|error| {
                        DistributedExecutionError::AgentUnavailable {
                            runtime_ref: runtime_ref.clone(),
                            error,
                        }
                    })?;

                    match response {
                        RuntimeAgentResponse::Executed { trace } => {
                            validate_segment_trace(&runtime_ref, &segment_plan, &trace)?;
                            trace
                        }
                        RuntimeAgentResponse::Rejected { error } => {
                            return Err(DistributedExecutionError::AgentRejected {
                                runtime_ref: runtime_ref.clone(),
                                error,
                            });
                        }
                    }
                }
            };

            for entry in &trace.entries {
                if entry.status == TraceStatus::Succeeded {
                    completed_stdout.insert(entry.step_id.clone(), entry.stdout.clone());
                    completed_runtime.insert(entry.step_id.clone(), runtime_ref.clone());
                }
                distributed_entries.push(DistributedTraceEntry {
                    runtime_ref: runtime_ref.clone(),
                    entry: entry.clone(),
                });
            }

            if !trace.succeeded() {
                for step in &placed.plan.steps[end..] {
                    let later_runtime_ref = placed
                        .runtime_for_step(&step.id)
                        .expect("validated placed Plan has runtime for every step")
                        .to_owned();
                    distributed_entries.push(DistributedTraceEntry {
                        runtime_ref: later_runtime_ref,
                        entry: trace_entry(
                            step,
                            TraceStatus::NotRun,
                            None,
                            String::new(),
                            String::new(),
                        ),
                    });
                }
                break;
            }

            start = end;
        }

        Ok(DistributedExecutionTrace {
            reference_graph_id: placed.plan.reference_graph_id.clone(),
            entries: distributed_entries,
            transfers,
        })
    }
}

fn validate_segment_trace(
    runtime_ref: &str,
    plan: &ExecutionPlan,
    trace: &ExecutionTrace,
) -> Result<(), DistributedExecutionError> {
    if trace.reference_graph_id != plan.reference_graph_id {
        return Err(DistributedExecutionError::MalformedAgentResponse {
            runtime_ref: runtime_ref.to_owned(),
            detail: format!(
                "reference Graph id '{}' does not match expected '{}'",
                trace.reference_graph_id, plan.reference_graph_id
            ),
        });
    }

    if trace.entries.len() != plan.steps.len() {
        return Err(DistributedExecutionError::MalformedAgentResponse {
            runtime_ref: runtime_ref.to_owned(),
            detail: format!(
                "entry count {} does not match submitted step count {}",
                trace.entries.len(),
                plan.steps.len()
            ),
        });
    }

    for (step, entry) in plan.steps.iter().zip(&trace.entries) {
        if entry.step_id != step.id {
            return Err(DistributedExecutionError::MalformedAgentResponse {
                runtime_ref: runtime_ref.to_owned(),
                detail: format!(
                    "trace step '{}' does not match submitted step '{}'",
                    entry.step_id, step.id
                ),
            });
        }
        if entry.implementation_ref != step.implementation_ref
            || entry.origin_block_ids != step.origin_block_ids
            || entry.source_anchors != step.source_anchors
            || entry.route_connector_ids != step.route_connector_ids
        {
            return Err(DistributedExecutionError::MalformedAgentResponse {
                runtime_ref: runtime_ref.to_owned(),
                detail: format!(
                    "trace attribution for step '{}' does not match submitted execution step",
                    step.id
                ),
            });
        }
    }

    Ok(())
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

    fn placement_test_plan() -> ExecutionPlan {
        ExecutionPlan {
            reference_graph_id: "graph:placement".to_owned(),
            steps: vec![
                ExecutionStep {
                    id: "step:local".to_owned(),
                    implementation_ref: "impl:local".to_owned(),
                    action: ProcessAction::new("local-program", std::iter::empty::<&str>()),
                    origin_block_ids: vec!["block:local".to_owned()],
                    source_anchors: Vec::new(),
                    route_connector_ids: Vec::new(),
                    argv_bindings: Vec::new(),
                },
                ExecutionStep {
                    id: "step:agent".to_owned(),
                    implementation_ref: "impl:agent".to_owned(),
                    action: ProcessAction::new("agent-program", std::iter::empty::<&str>()),
                    origin_block_ids: vec!["block:agent".to_owned()],
                    source_anchors: Vec::new(),
                    route_connector_ids: Vec::new(),
                    argv_bindings: Vec::new(),
                },
            ],
        }
    }

    fn placement_test_endpoints() -> Vec<RuntimeEndpoint> {
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

    fn valid_placed_plan() -> PlacedExecutionPlan {
        PlacedExecutionPlan::new(
            placement_test_plan(),
            [
                StepPlacement::new("step:local", "runtime:local"),
                StepPlacement::new("step:agent", "runtime:agent"),
            ],
        )
    }

    #[test]
    fn placed_plan_validates_multi_endpoint_assignment_and_round_trips() {
        let placed = valid_placed_plan();
        let endpoints = placement_test_endpoints();

        placed.validate(&endpoints).unwrap();
        assert_eq!(placed.runtime_for_step("step:local"), Some("runtime:local"));
        assert_eq!(placed.runtime_for_step("step:agent"), Some("runtime:agent"));

        let original_plan_json = serde_json::to_value(&placed.plan).unwrap();
        assert!(original_plan_json.get("placements").is_none());
        assert!(original_plan_json.get("runtime_ref").is_none());

        let serialized = serde_json::to_string(&placed).unwrap();
        let restored: PlacedExecutionPlan = serde_json::from_str(&serialized).unwrap();

        assert_eq!(restored, placed);
        assert_eq!(restored.plan, placed.plan);
        assert_eq!(
            serde_json::to_value(&restored.plan).unwrap(),
            original_plan_json
        );
        restored.validate(&endpoints).unwrap();
    }

    #[test]
    fn placed_plan_rejects_missing_duplicate_and_unknown_step_placements() {
        let endpoints = placement_test_endpoints();

        let missing = PlacedExecutionPlan::new(
            placement_test_plan(),
            [StepPlacement::new("step:local", "runtime:local")],
        );
        assert!(matches!(
            missing.validate(&endpoints),
            Err(PlacementError::MissingPlacement { step_id })
                if step_id == "step:agent"
        ));

        let duplicate = PlacedExecutionPlan::new(
            placement_test_plan(),
            [
                StepPlacement::new("step:local", "runtime:local"),
                StepPlacement::new("step:local", "runtime:local"),
                StepPlacement::new("step:agent", "runtime:agent"),
            ],
        );
        assert!(matches!(
            duplicate.validate(&endpoints),
            Err(PlacementError::DuplicatePlacement { step_id })
                if step_id == "step:local"
        ));

        let unknown = PlacedExecutionPlan::new(
            placement_test_plan(),
            [
                StepPlacement::new("step:local", "runtime:local"),
                StepPlacement::new("step:agent", "runtime:agent"),
                StepPlacement::new("step:ghost", "runtime:local"),
            ],
        );
        assert!(matches!(
            unknown.validate(&endpoints),
            Err(PlacementError::UnknownPlacementStep { step_id })
                if step_id == "step:ghost"
        ));
    }

    #[test]
    fn placed_plan_rejects_unknown_runtime_and_unavailable_implementation() {
        let endpoints = placement_test_endpoints();

        let unknown_runtime = PlacedExecutionPlan::new(
            placement_test_plan(),
            [
                StepPlacement::new("step:local", "runtime:missing"),
                StepPlacement::new("step:agent", "runtime:agent"),
            ],
        );
        assert!(matches!(
            unknown_runtime.validate(&endpoints),
            Err(PlacementError::UnknownRuntime {
                step_id,
                runtime_ref,
            }) if step_id == "step:local" && runtime_ref == "runtime:missing"
        ));

        let unavailable = PlacedExecutionPlan::new(
            placement_test_plan(),
            [
                StepPlacement::new("step:local", "runtime:agent"),
                StepPlacement::new("step:agent", "runtime:local"),
            ],
        );
        assert!(matches!(
            unavailable.validate(&endpoints),
            Err(PlacementError::ImplementationUnavailable {
                step_id,
                runtime_ref,
                implementation_ref,
            }) if step_id == "step:local"
                && runtime_ref == "runtime:agent"
                && implementation_ref == "impl:local"
        ));
    }

    #[test]
    fn placed_plan_rejects_ambiguous_plan_or_runtime_identity() {
        let mut duplicate_step_plan = placement_test_plan();
        duplicate_step_plan.steps[1].id = "step:local".to_owned();
        let duplicate_step = PlacedExecutionPlan::new(
            duplicate_step_plan,
            [StepPlacement::new("step:local", "runtime:local")],
        );
        assert!(matches!(
            duplicate_step.validate(&placement_test_endpoints()),
            Err(PlacementError::DuplicatePlanStepId(step_id))
                if step_id == "step:local"
        ));

        let duplicate_runtime = vec![
            RuntimeEndpoint::new(
                "runtime:same",
                RuntimeLocationClass::LocalProcess,
                ["impl:local"],
            ),
            RuntimeEndpoint::new(
                "runtime:same",
                RuntimeLocationClass::RuntimeAgent,
                ["impl:agent"],
            ),
        ];
        assert!(matches!(
            valid_placed_plan().validate(&duplicate_runtime),
            Err(PlacementError::DuplicateRuntimeRef(runtime_ref))
                if runtime_ref == "runtime:same"
        ));
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

    fn marker_action(marker: &Path, value: &str) -> ProcessAction {
        ProcessAction::new(
            "python3",
            [
                "-c".to_owned(),
                "from pathlib import Path; import sys; Path(sys.argv[1]).write_text(sys.argv[2])"
                    .to_owned(),
                marker.display().to_string(),
                value.to_owned(),
            ],
        )
    }

    fn single_step_plan(implementation_ref: &str, action: ProcessAction) -> ExecutionPlan {
        ExecutionPlan {
            reference_graph_id: "graph:guarded".to_owned(),
            steps: vec![ExecutionStep {
                id: "step:guarded".to_owned(),
                implementation_ref: implementation_ref.to_owned(),
                action,
                origin_block_ids: vec!["block:guarded".to_owned()],
                source_anchors: Vec::new(),
                route_connector_ids: Vec::new(),
                argv_bindings: Vec::new(),
            }],
        }
    }

    #[test]
    fn guarded_inspection_reports_actual_ambient_host_action() {
        let action = ProcessAction::new(
            "python3",
            ["-c".to_owned(), "print('GUARDED_INSPECT')".to_owned()],
        );
        let plan = single_step_plan("impl:guarded", action.clone());

        let mut trusted = ImplementationRegistry::new();
        trusted.register("impl:guarded", action.clone()).unwrap();

        let report = GuardedProcessRuntime::inspect(&plan, &trusted).unwrap();

        assert_eq!(report.reference_graph_id, "graph:guarded");
        assert_eq!(report.requirements.len(), 1);
        let requirement = &report.requirements[0];
        assert_eq!(requirement.step_id, "step:guarded");
        assert_eq!(requirement.implementation_ref, "impl:guarded");
        assert_eq!(requirement.origin_block_ids, vec!["block:guarded"]);
        assert_eq!(
            requirement.access_class,
            ExecutionAccessClass::AmbientHostProcess
        );
        assert_eq!(requirement.action, action);
        assert!(requirement.argv_bindings.is_empty());
    }

    #[test]
    fn guarded_execution_is_default_deny_and_explicit_allow_succeeds() {
        let action = ProcessAction::new(
            "python3",
            ["-c".to_owned(), "print('GUARDED_OK')".to_owned()],
        );
        let plan = single_step_plan("impl:guarded", action.clone());

        let mut trusted = ImplementationRegistry::new();
        trusted.register("impl:guarded", action).unwrap();

        let denied =
            GuardedProcessRuntime::execute(&plan, &trusted, &ExecutionPolicy::new()).unwrap_err();
        assert!(matches!(
            denied,
            ExecutionSecurityError::DeniedImplementation { .. }
        ));

        let mut policy = ExecutionPolicy::new();
        policy.allow("impl:guarded");
        let trace = GuardedProcessRuntime::execute(&plan, &trusted, &policy).unwrap();
        assert!(trace.succeeded());
        assert_eq!(trace.entries[0].implementation_ref, "impl:guarded");
        assert_eq!(trace.entries[0].origin_block_ids, vec!["block:guarded"]);
        assert_eq!(trace.entries[0].stdout.trim(), "GUARDED_OK");
    }

    #[test]
    fn guarded_full_plan_denial_prevents_all_process_launches() {
        let marker_first = std::env::temp_dir().join(format!(
            "algoram-guarded-first-must-not-run-{}",
            std::process::id()
        ));
        let marker_second = std::env::temp_dir().join(format!(
            "algoram-guarded-second-must-not-run-{}",
            std::process::id()
        ));
        let _ = fs::remove_file(&marker_first);
        let _ = fs::remove_file(&marker_second);

        let first_action = marker_action(&marker_first, "first");
        let second_action = marker_action(&marker_second, "second");
        let plan = ExecutionPlan {
            reference_graph_id: "graph:guarded-full-plan".to_owned(),
            steps: vec![
                ExecutionStep {
                    id: "step:first".to_owned(),
                    implementation_ref: "impl:first".to_owned(),
                    action: first_action.clone(),
                    origin_block_ids: vec!["block:first".to_owned()],
                    source_anchors: Vec::new(),
                    route_connector_ids: Vec::new(),
                    argv_bindings: Vec::new(),
                },
                ExecutionStep {
                    id: "step:second".to_owned(),
                    implementation_ref: "impl:second".to_owned(),
                    action: second_action.clone(),
                    origin_block_ids: vec!["block:second".to_owned()],
                    source_anchors: Vec::new(),
                    route_connector_ids: Vec::new(),
                    argv_bindings: Vec::new(),
                },
            ],
        };

        let mut trusted = ImplementationRegistry::new();
        trusted.register("impl:first", first_action).unwrap();
        trusted.register("impl:second", second_action).unwrap();

        let mut policy = ExecutionPolicy::new();
        policy.allow("impl:first");

        let error = GuardedProcessRuntime::execute(&plan, &trusted, &policy).unwrap_err();
        assert!(matches!(
            error,
            ExecutionSecurityError::DeniedImplementation {
                implementation_ref,
                ..
            } if implementation_ref == "impl:second"
        ));
        assert!(!marker_first.exists());
        assert!(!marker_second.exists());
    }

    #[test]
    fn guarded_missing_or_tampered_action_fails_before_launch() {
        let marker = std::env::temp_dir().join(format!(
            "algoram-guarded-tampered-must-not-run-{}",
            std::process::id()
        ));
        let _ = fs::remove_file(&marker);

        let tampered_action = marker_action(&marker, "tampered");
        let plan = single_step_plan("impl:guarded", tampered_action);

        let trusted_action =
            ProcessAction::new("python3", ["-c".to_owned(), "print('TRUSTED')".to_owned()]);
        let mut trusted = ImplementationRegistry::new();
        trusted.register("impl:guarded", trusted_action).unwrap();

        let mut policy = ExecutionPolicy::new();
        policy.allow("impl:guarded");

        let tampered = GuardedProcessRuntime::execute(&plan, &trusted, &policy).unwrap_err();
        assert!(matches!(
            tampered,
            ExecutionSecurityError::TrustedActionMismatch { .. }
        ));
        assert!(!marker.exists());

        let missing =
            GuardedProcessRuntime::inspect(&plan, &ImplementationRegistry::new()).unwrap_err();
        assert!(matches!(
            missing,
            ExecutionSecurityError::MissingTrustedImplementation { .. }
        ));
        assert!(!marker.exists());
    }

    #[test]
    fn guarded_argv_binding_shape_cannot_exceed_trusted_action_contract() {
        let trusted_action = ProcessAction::new(
            "python3",
            ["-c".to_owned(), "print('BINDING_SHAPE')".to_owned()],
        )
        .with_argv_ports(["value"]);
        let plan = single_step_plan("impl:guarded", trusted_action.clone());

        let mut trusted = ImplementationRegistry::new();
        trusted.register("impl:guarded", trusted_action).unwrap();

        let error = GuardedProcessRuntime::inspect(&plan, &trusted).unwrap_err();
        assert!(matches!(
            error,
            ExecutionSecurityError::ArgvBindingShapeMismatch {
                expected_ports,
                found_ports,
                ..
            } if expected_ports == vec!["value"] && found_ports.is_empty()
        ));
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

    fn route_override_graph() -> Graph {
        let mut graph = Graph::new("graph:manual-route");
        let mut source = block("block:route-source", None);
        source.ports.push(port(
            "out",
            PortDirection::Out,
            PortChannel::Data,
            Some("contract:a"),
        ));
        let mut target = block("block:route-target", Some("impl:route-target"));
        target.ports.push(port(
            "in",
            PortDirection::In,
            PortChannel::Data,
            Some("contract:c"),
        ));
        graph.blocks.extend([source, target]);
        graph.connections.push(Connection {
            id: "data:manual-route".to_owned(),
            source: PortRef {
                block_id: "block:route-source".to_owned(),
                port_id: "out".to_owned(),
            },
            target: PortRef {
                block_id: "block:route-target".to_owned(),
                port_id: "in".to_owned(),
            },
            extensions: Extensions::new(),
        });
        graph
    }

    fn route_override_implementations(action: ProcessAction) -> ImplementationRegistry {
        let mut implementations = ImplementationRegistry::new();
        implementations
            .register("impl:route-target", action)
            .unwrap();
        implementations
    }

    fn route_override_registry() -> RouteRegistry {
        let mut routes = RouteRegistry::new();
        routes
            .register(
                Connector::new(
                    "primary",
                    ContractId::from("contract:a"),
                    ContractId::from("contract:c"),
                    "impl:primary-route",
                )
                .unavailable(),
            )
            .unwrap();
        routes
            .register(Connector::new(
                "a-1",
                ContractId::from("contract:a"),
                ContractId::from("contract:b"),
                "impl:a-1",
            ))
            .unwrap();
        routes
            .register(Connector::new(
                "a-2",
                ContractId::from("contract:b"),
                ContractId::from("contract:c"),
                "impl:a-2",
            ))
            .unwrap();
        routes
            .register(Connector::new(
                "z-1",
                ContractId::from("contract:a"),
                ContractId::from("contract:d"),
                "impl:z-1",
            ))
            .unwrap();
        routes
            .register(Connector::new(
                "z-2",
                ContractId::from("contract:d"),
                ContractId::from("contract:c"),
                "impl:z-2",
            ))
            .unwrap();
        routes
    }

    #[test]
    fn explicit_route_override_can_choose_valid_non_default_route_on_same_graph() {
        let graph = route_override_graph();
        let graph_before = graph.clone();
        let implementations = route_override_implementations(ProcessAction::new(
            "target",
            std::iter::empty::<&str>(),
        ));
        let routes = route_override_registry();

        let automatic = Planner::lower(&graph, &implementations, &routes).unwrap();
        assert_eq!(automatic.steps[0].route_connector_ids, vec!["a-1", "a-2"]);

        let manual = Planner::lower_with_route_overrides(
            &graph,
            &implementations,
            &routes,
            &[RouteOverride::new("data:manual-route", ["z-1", "z-2"])],
        )
        .unwrap();

        assert_eq!(graph, graph_before);
        assert_eq!(manual.steps[0].route_connector_ids, vec!["z-1", "z-2"]);

        let serialized = serde_json::to_string(&manual).unwrap();
        let replayed: ExecutionPlan = serde_json::from_str(&serialized).unwrap();
        assert_eq!(replayed.steps[0].route_connector_ids, vec!["z-1", "z-2"]);
    }

    #[test]
    fn unavailable_explicit_route_fails_during_planning_without_execution() {
        let marker = std::env::temp_dir().join(format!(
            "algoram-route-override-must-not-execute-{}",
            std::process::id()
        ));
        let _ = fs::remove_file(&marker);

        let graph = route_override_graph();
        let implementations = route_override_implementations(ProcessAction::new(
            "sh",
            ["-c".to_owned(), format!("touch {}", marker.display())],
        ));
        let routes = route_override_registry();

        let error = Planner::lower_with_route_overrides(
            &graph,
            &implementations,
            &routes,
            &[RouteOverride::new("data:manual-route", ["primary"])],
        )
        .unwrap_err();

        assert!(matches!(
            error,
            PlannerError::Route(RouteError::ConnectorUnavailable(id)) if id == "primary"
        ));
        assert!(!marker.exists());
    }

    #[test]
    fn route_override_identity_errors_are_visible() {
        let graph = route_override_graph();
        let implementations = route_override_implementations(ProcessAction::new(
            "target",
            std::iter::empty::<&str>(),
        ));
        let routes = route_override_registry();

        let unknown = Planner::lower_with_route_overrides(
            &graph,
            &implementations,
            &routes,
            &[RouteOverride::new("data:missing", ["z-1", "z-2"])],
        )
        .unwrap_err();
        assert!(matches!(
            unknown,
            PlannerError::UnknownRouteOverrideConnection { connection_id }
                if connection_id == "data:missing"
        ));

        let duplicate = Planner::lower_with_route_overrides(
            &graph,
            &implementations,
            &routes,
            &[
                RouteOverride::new("data:manual-route", ["z-1", "z-2"]),
                RouteOverride::new("data:manual-route", ["a-1", "a-2"]),
            ],
        )
        .unwrap_err();
        assert!(matches!(
            duplicate,
            PlannerError::DuplicateRouteOverride { connection_id }
                if connection_id == "data:manual-route"
        ));
    }

    #[test]
    fn route_override_on_same_contract_connection_is_not_applicable() {
        let mut graph = Graph::new("graph:route-override-not-applicable");
        let mut source = block("block:same-source", None);
        source.ports.push(port(
            "out",
            PortDirection::Out,
            PortChannel::Data,
            Some("contract:same"),
        ));
        let mut target = block("block:same-target", Some("impl:same-target"));
        target.ports.push(port(
            "in",
            PortDirection::In,
            PortChannel::Data,
            Some("contract:same"),
        ));
        graph.blocks.extend([source, target]);
        graph.connections.push(Connection {
            id: "data:same".to_owned(),
            source: PortRef {
                block_id: "block:same-source".to_owned(),
                port_id: "out".to_owned(),
            },
            target: PortRef {
                block_id: "block:same-target".to_owned(),
                port_id: "in".to_owned(),
            },
            extensions: Extensions::new(),
        });

        let mut implementations = ImplementationRegistry::new();
        implementations
            .register(
                "impl:same-target",
                ProcessAction::new("target", std::iter::empty::<&str>()),
            )
            .unwrap();

        let error = Planner::lower_with_route_overrides(
            &graph,
            &implementations,
            &RouteRegistry::new(),
            &[RouteOverride::new("data:same", ["unused"])],
        )
        .unwrap_err();

        assert!(matches!(
            error,
            PlannerError::RouteOverrideNotApplicable { connection_id }
                if connection_id == "data:same"
        ));
    }
}
