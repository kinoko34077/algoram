use algoram_core::{
    Block, Connection, Extensions, Graph, Port, PortChannel, PortDirection, PortRef,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::fmt;

pub const DEFAULT_MAX_HOPS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContractId(String);

impl ContractId {
    pub fn new(value: impl Into<String>) -> Result<Self, RouteError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(RouteError::EmptyContract);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ContractId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferMode {
    Copy,
    Borrow,
    Move,
    Proxy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchedulingMode {
    Sync,
    Async,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connector {
    pub id: String,
    pub source: ContractId,
    pub target: ContractId,
    pub implementation_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transfer: Option<TransferMode>,
    pub scheduling: SchedulingMode,
    #[serde(default = "default_available")]
    pub available: bool,
}

fn default_available() -> bool {
    true
}

impl Connector {
    pub fn new(
        id: impl Into<String>,
        source: impl Into<ContractId>,
        target: impl Into<ContractId>,
        implementation_ref: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
            target: target.into(),
            implementation_ref: implementation_ref.into(),
            transfer: None,
            scheduling: SchedulingMode::Sync,
            available: true,
        }
    }

    pub fn with_transfer(mut self, transfer: TransferMode) -> Self {
        self.transfer = Some(transfer);
        self
    }

    pub fn with_scheduling(mut self, scheduling: SchedulingMode) -> Self {
        self.scheduling = scheduling;
        self
    }

    pub fn unavailable(mut self) -> Self {
        self.available = false;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedRoute {
    pub source: ContractId,
    pub target: ContractId,
    pub steps: Vec<Connector>,
}

impl ResolvedRoute {
    pub fn is_direct(&self) -> bool {
        self.steps.len() == 1
    }

    pub fn connector_ids(&self) -> Vec<&str> {
        self.steps.iter().map(|step| step.id.as_str()).collect()
    }

    pub fn to_inspection_graph(&self) -> Result<Graph, RouteError> {
        if self.steps.is_empty() {
            return Err(RouteError::EmptyResolvedRoute);
        }

        let mut graph = Graph::new(format!(
            "graph:interop:{}->{}",
            stable_fragment(self.source.as_str()),
            stable_fragment(self.target.as_str())
        ));
        graph.label = Some(format!(
            "Interop route: {} → {}",
            self.source.as_str(),
            self.target.as_str()
        ));

        for step in &self.steps {
            let mut extensions = Extensions::new();
            extensions.insert(
                "interop".to_owned(),
                json!({
                    "connector_id": step.id,
                    "source_contract": step.source.as_str(),
                    "target_contract": step.target.as_str(),
                    "implementation_ref": step.implementation_ref,
                    "transfer": step.transfer,
                    "scheduling": step.scheduling
                }),
            );

            graph.blocks.push(Block {
                id: format!("interop:connector:{}", stable_fragment(&step.id)),
                label: step.id.clone(),
                ports: vec![
                    Port {
                        id: "in".to_owned(),
                        direction: PortDirection::In,
                        channel: PortChannel::Data,
                        contract: Some(json!(step.source.as_str())),
                        extensions: Extensions::new(),
                    },
                    Port {
                        id: "out".to_owned(),
                        direction: PortDirection::Out,
                        channel: PortChannel::Data,
                        contract: Some(json!(step.target.as_str())),
                        extensions: Extensions::new(),
                    },
                ],
                internal_graph_ref: None,
                implementation_ref: Some(step.implementation_ref.clone()),
                definition_ref: None,
                source_anchor: None,
                extensions,
                diagnostics: Vec::new(),
            });
        }

        for window in graph.blocks.windows(2) {
            let source = &window[0];
            let target = &window[1];
            graph.connections.push(Connection {
                id: format!("route:{}->{}", source.id, target.id),
                source: PortRef {
                    block_id: source.id.clone(),
                    port_id: "out".to_owned(),
                },
                target: PortRef {
                    block_id: target.id.clone(),
                    port_id: "in".to_owned(),
                },
                extensions: Extensions::new(),
            });
        }

        graph.validate().map_err(RouteError::InspectionGraph)?;
        Ok(graph)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    Resolved {
        route: ResolvedRoute,
        direct_route_available: bool,
    },
    Unresolved {
        source: ContractId,
        target: ContractId,
        direct_route_available: bool,
    },
}

impl Resolution {
    pub fn route(&self) -> Option<&ResolvedRoute> {
        match self {
            Self::Resolved { route, .. } => Some(route),
            Self::Unresolved { .. } => None,
        }
    }

    pub fn direct_route_available(&self) -> bool {
        match self {
            Self::Resolved {
                direct_route_available,
                ..
            }
            | Self::Unresolved {
                direct_route_available,
                ..
            } => *direct_route_available,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteRequest {
    pub source: ContractId,
    pub target: ContractId,
    pub explicit_connector_ids: Option<Vec<String>>,
    pub max_hops: usize,
}

impl RouteRequest {
    pub fn automatic(source: impl Into<ContractId>, target: impl Into<ContractId>) -> Self {
        Self {
            source: source.into(),
            target: target.into(),
            explicit_connector_ids: None,
            max_hops: DEFAULT_MAX_HOPS,
        }
    }

    pub fn explicit(
        source: impl Into<ContractId>,
        target: impl Into<ContractId>,
        connector_ids: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            source: source.into(),
            target: target.into(),
            explicit_connector_ids: Some(
                connector_ids.into_iter().map(Into::into).collect(),
            ),
            max_hops: DEFAULT_MAX_HOPS,
        }
    }

    pub fn with_max_hops(mut self, max_hops: usize) -> Self {
        self.max_hops = max_hops;
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct RouteRegistry {
    connectors: Vec<Connector>,
}

impl RouteRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, connector: Connector) -> Result<(), RouteError> {
        if connector.id.trim().is_empty() {
            return Err(RouteError::EmptyConnectorId);
        }
        if self
            .connectors
            .iter()
            .any(|existing| existing.id == connector.id)
        {
            return Err(RouteError::DuplicateConnectorId(connector.id));
        }
        self.connectors.push(connector);
        self.connectors.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(())
    }

    pub fn connectors(&self) -> &[Connector] {
        &self.connectors
    }

    pub fn has_direct_route(&self, source: &ContractId, target: &ContractId) -> bool {
        self.connectors.iter().any(|connector| {
            connector.available && &connector.source == source && &connector.target == target
        })
    }

    pub fn resolve(&self, request: &RouteRequest) -> Result<Resolution, RouteError> {
        if request.max_hops == 0 {
            if request.source == request.target {
                return Ok(Resolution::Resolved {
                    route: ResolvedRoute {
                        source: request.source.clone(),
                        target: request.target.clone(),
                        steps: Vec::new(),
                    },
                    direct_route_available: false,
                });
            }
            return Ok(Resolution::Unresolved {
                source: request.source.clone(),
                target: request.target.clone(),
                direct_route_available: false,
            });
        }

        let direct_route_available = self.has_direct_route(&request.source, &request.target);

        if let Some(explicit_ids) = &request.explicit_connector_ids {
            let route = self.validate_explicit(request, explicit_ids)?;
            return Ok(Resolution::Resolved {
                route,
                direct_route_available,
            });
        }

        if request.source == request.target {
            return Ok(Resolution::Resolved {
                route: ResolvedRoute {
                    source: request.source.clone(),
                    target: request.target.clone(),
                    steps: Vec::new(),
                },
                direct_route_available,
            });
        }

        if let Some(connector) = self
            .connectors
            .iter()
            .filter(|connector| {
                connector.available
                    && connector.source == request.source
                    && connector.target == request.target
            })
            .min_by(|a, b| a.id.cmp(&b.id))
        {
            return Ok(Resolution::Resolved {
                route: ResolvedRoute {
                    source: request.source.clone(),
                    target: request.target.clone(),
                    steps: vec![connector.clone()],
                },
                direct_route_available: true,
            });
        }

        let route = self.resolve_indirect(request)?;
        Ok(match route {
            Some(route) => Resolution::Resolved {
                route,
                direct_route_available,
            },
            None => Resolution::Unresolved {
                source: request.source.clone(),
                target: request.target.clone(),
                direct_route_available,
            },
        })
    }

    fn validate_explicit(
        &self,
        request: &RouteRequest,
        connector_ids: &[String],
    ) -> Result<ResolvedRoute, RouteError> {
        if connector_ids.is_empty() {
            if request.source == request.target {
                return Ok(ResolvedRoute {
                    source: request.source.clone(),
                    target: request.target.clone(),
                    steps: Vec::new(),
                });
            }
            return Err(RouteError::ExplicitRouteEmpty);
        }

        if connector_ids.len() > request.max_hops {
            return Err(RouteError::ExplicitRouteExceedsMaxHops {
                hops: connector_ids.len(),
                max_hops: request.max_hops,
            });
        }

        let by_id: HashMap<&str, &Connector> = self
            .connectors
            .iter()
            .map(|connector| (connector.id.as_str(), connector))
            .collect();

        let mut current = request.source.clone();
        let mut steps = Vec::with_capacity(connector_ids.len());

        for connector_id in connector_ids {
            let connector = by_id
                .get(connector_id.as_str())
                .copied()
                .ok_or_else(|| RouteError::UnknownConnector(connector_id.clone()))?;

            if !connector.available {
                return Err(RouteError::ConnectorUnavailable(connector.id.clone()));
            }

            if connector.source != current {
                return Err(RouteError::ExplicitRouteBrokenChain {
                    connector_id: connector.id.clone(),
                    expected_source: current,
                    found_source: connector.source.clone(),
                });
            }

            current = connector.target.clone();
            steps.push(connector.clone());
        }

        if current != request.target {
            return Err(RouteError::ExplicitRouteWrongTarget {
                expected: request.target.clone(),
                found: current,
            });
        }

        Ok(ResolvedRoute {
            source: request.source.clone(),
            target: request.target.clone(),
            steps,
        })
    }

    fn resolve_indirect(
        &self,
        request: &RouteRequest,
    ) -> Result<Option<ResolvedRoute>, RouteError> {
        #[derive(Clone)]
        struct State {
            contract: ContractId,
            steps: Vec<Connector>,
            visited: Vec<ContractId>,
        }

        let mut queue = VecDeque::new();
        queue.push_back(State {
            contract: request.source.clone(),
            steps: Vec::new(),
            visited: vec![request.source.clone()],
        });

        while let Some(state) = queue.pop_front() {
            if state.steps.len() >= request.max_hops {
                continue;
            }

            let mut outgoing = self
                .connectors
                .iter()
                .filter(|connector| connector.available && connector.source == state.contract)
                .collect::<Vec<_>>();
            outgoing.sort_by(|a, b| a.id.cmp(&b.id));

            for connector in outgoing {
                if state.visited.contains(&connector.target) {
                    continue;
                }

                let mut steps = state.steps.clone();
                steps.push(connector.clone());

                if connector.target == request.target {
                    return Ok(Some(ResolvedRoute {
                        source: request.source.clone(),
                        target: request.target.clone(),
                        steps,
                    }));
                }

                let mut visited = state.visited.clone();
                visited.push(connector.target.clone());
                queue.push_back(State {
                    contract: connector.target.clone(),
                    steps,
                    visited,
                });
            }
        }

        Ok(None)
    }
}

#[derive(Debug)]
pub enum RouteError {
    EmptyContract,
    EmptyConnectorId,
    DuplicateConnectorId(String),
    UnknownConnector(String),
    ConnectorUnavailable(String),
    ExplicitRouteEmpty,
    ExplicitRouteExceedsMaxHops {
        hops: usize,
        max_hops: usize,
    },
    ExplicitRouteBrokenChain {
        connector_id: String,
        expected_source: ContractId,
        found_source: ContractId,
    },
    ExplicitRouteWrongTarget {
        expected: ContractId,
        found: ContractId,
    },
    EmptyResolvedRoute,
    InspectionGraph(algoram_core::GraphError),
}

impl fmt::Display for RouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyContract => write!(f, "contract identifier must not be empty"),
            Self::EmptyConnectorId => write!(f, "connector identifier must not be empty"),
            Self::DuplicateConnectorId(id) => write!(f, "duplicate connector id '{id}'"),
            Self::UnknownConnector(id) => write!(f, "unknown connector '{id}'"),
            Self::ConnectorUnavailable(id) => write!(f, "connector '{id}' is unavailable"),
            Self::ExplicitRouteEmpty => {
                write!(f, "explicit route cannot be empty when source != target")
            }
            Self::ExplicitRouteExceedsMaxHops { hops, max_hops } => write!(
                f,
                "explicit route has {hops} hops, exceeding max_hops={max_hops}"
            ),
            Self::ExplicitRouteBrokenChain {
                connector_id,
                expected_source,
                found_source,
            } => write!(
                f,
                "explicit route connector '{connector_id}' expected source '{}', found '{}'",
                expected_source.as_str(),
                found_source.as_str()
            ),
            Self::ExplicitRouteWrongTarget { expected, found } => write!(
                f,
                "explicit route ended at '{}', expected '{}'",
                found.as_str(),
                expected.as_str()
            ),
            Self::EmptyResolvedRoute => {
                write!(f, "zero-hop route has no connector graph to inspect")
            }
            Self::InspectionGraph(error) => write!(f, "invalid inspection graph: {error}"),
        }
    }
}

impl Error for RouteError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InspectionGraph(error) => Some(error),
            _ => None,
        }
    }
}

fn stable_fragment(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for character in input.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.') {
            output.push(character);
        } else {
            output.push('_');
        }
    }
    if output.is_empty() {
        "route".to_owned()
    } else {
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connector(id: &str, source: &str, target: &str) -> Connector {
        Connector::new(id, ContractId::from(source), ContractId::from(target), format!("impl:{id}"))
            .with_transfer(TransferMode::Copy)
            .with_scheduling(SchedulingMode::Sync)
    }

    #[test]
    fn direct_route_is_preferred_and_deterministic() {
        let mut registry = RouteRegistry::new();
        registry.register(connector("z-direct", "a", "b")).unwrap();
        registry.register(connector("a-direct", "a", "b")).unwrap();
        registry.register(connector("indirect-1", "a", "x")).unwrap();
        registry.register(connector("indirect-2", "x", "b")).unwrap();

        let result = registry
            .resolve(&RouteRequest::automatic("a", "b"))
            .unwrap();
        let route = result.route().unwrap();

        assert_eq!(route.connector_ids(), vec!["a-direct"]);
        assert!(route.is_direct());
        assert!(result.direct_route_available());
    }

    #[test]
    fn indirect_route_uses_fewest_hops_then_lexical_connector_order() {
        let mut registry = RouteRegistry::new();
        registry.register(connector("b-first", "a", "x")).unwrap();
        registry.register(connector("b-second", "x", "d")).unwrap();
        registry.register(connector("a-first", "a", "y")).unwrap();
        registry.register(connector("a-second", "y", "d")).unwrap();
        registry.register(connector("long-1", "a", "p")).unwrap();
        registry.register(connector("long-2", "p", "q")).unwrap();
        registry.register(connector("long-3", "q", "d")).unwrap();

        let route = registry
            .resolve(&RouteRequest::automatic("a", "d"))
            .unwrap()
            .route()
            .unwrap()
            .clone();

        assert_eq!(route.connector_ids(), vec!["a-first", "a-second"]);
        assert!(!route.is_direct());
    }

    #[test]
    fn explicit_valid_route_overrides_shorter_automatic_route() {
        let mut registry = RouteRegistry::new();
        registry.register(connector("direct", "a", "d")).unwrap();
        registry.register(connector("via-b", "a", "b")).unwrap();
        registry.register(connector("b-to-d", "b", "d")).unwrap();

        let request = RouteRequest::explicit("a", "d", ["via-b", "b-to-d"]);
        let route = registry.resolve(&request).unwrap().route().unwrap().clone();

        assert_eq!(route.connector_ids(), vec!["via-b", "b-to-d"]);
    }

    #[test]
    fn invalid_explicit_route_is_rejected() {
        let mut registry = RouteRegistry::new();
        registry.register(connector("a-b", "a", "b")).unwrap();
        registry.register(connector("c-d", "c", "d")).unwrap();

        let error = registry
            .resolve(&RouteRequest::explicit("a", "d", ["a-b", "c-d"]))
            .unwrap_err();

        assert!(matches!(error, RouteError::ExplicitRouteBrokenChain { .. }));
    }

    #[test]
    fn cycle_is_skipped_and_search_is_bounded() {
        let mut registry = RouteRegistry::new();
        registry.register(connector("a-b", "a", "b")).unwrap();
        registry.register(connector("b-a", "b", "a")).unwrap();
        registry.register(connector("b-c", "b", "c")).unwrap();

        let route = registry
            .resolve(&RouteRequest::automatic("a", "c").with_max_hops(2))
            .unwrap()
            .route()
            .unwrap()
            .clone();
        assert_eq!(route.connector_ids(), vec!["a-b", "b-c"]);

        let unresolved = registry
            .resolve(&RouteRequest::automatic("a", "c").with_max_hops(1))
            .unwrap();
        assert!(matches!(unresolved, Resolution::Unresolved { .. }));
    }

    #[test]
    fn unavailable_connector_does_not_make_a_route() {
        let mut registry = RouteRegistry::new();
        registry
            .register(connector("offline", "a", "b").unavailable())
            .unwrap();

        let result = registry
            .resolve(&RouteRequest::automatic("a", "b"))
            .unwrap();

        assert!(matches!(result, Resolution::Unresolved { .. }));
        assert!(!result.direct_route_available());
    }

    #[test]
    fn reachability_does_not_encode_equivalence_or_substitution() {
        let connector = connector(
            "python-ctypes-c-abi-int32",
            "python:ctypes:c_int",
            "c:abi:int32",
        );
        let serialized = serde_json::to_value(&connector).unwrap();

        assert!(serialized.get("equivalent").is_none());
        assert!(serialized.get("substitutable").is_none());
    }

    #[test]
    fn transfer_and_scheduling_are_preserved_in_inspection_graph() {
        let mut registry = RouteRegistry::new();
        registry
            .register(
                Connector::new(
                    "python-ctypes-c-abi-int32",
                    ContractId::from("python:ctypes:c_int"),
                    ContractId::from("c:abi:int32"),
                    "fixture:python-c-ctypes",
                )
                .with_transfer(TransferMode::Copy)
                .with_scheduling(SchedulingMode::Sync),
            )
            .unwrap();

        let route = registry
            .resolve(&RouteRequest::automatic(
                "python:ctypes:c_int",
                "c:abi:int32",
            ))
            .unwrap()
            .route()
            .unwrap()
            .clone();

        let graph = route.to_inspection_graph().unwrap();
        assert_eq!(graph.blocks.len(), 1);
        assert_eq!(
            graph.blocks[0].extensions["interop"]["transfer"],
            json!("copy")
        );
        assert_eq!(
            graph.blocks[0].extensions["interop"]["scheduling"],
            json!("sync")
        );
        assert_eq!(
            graph.blocks[0].implementation_ref.as_deref(),
            Some("fixture:python-c-ctypes")
        );
    }

    #[test]
    fn python_ctypes_to_c_function_route_is_inspectable() {
        let mut registry = RouteRegistry::new();
        registry
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
        registry
            .register(
                Connector::new(
                    "c-abi-call-algoram-double",
                    ContractId::from("c:abi:int32"),
                    ContractId::from("c:function:algoram_double:int32"),
                    "fixture:c:function:algoram_double",
                )
                .with_transfer(TransferMode::Copy),
            )
            .unwrap();

        let route = registry
            .resolve(&RouteRequest::automatic(
                "python:ctypes:c_int",
                "c:function:algoram_double:int32",
            ))
            .unwrap()
            .route()
            .unwrap()
            .clone();

        assert_eq!(
            route.connector_ids(),
            vec![
                "python-ctypes-c-abi-int32",
                "c-abi-call-algoram-double"
            ]
        );

        let graph = route.to_inspection_graph().unwrap();
        assert_eq!(graph.blocks.len(), 2);
        assert_eq!(graph.connections.len(), 1);
        graph.validate().unwrap();
    }
}
