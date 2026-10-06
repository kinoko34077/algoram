use algoram_catalog::{CapabilityCatalog, CatalogAsset, CatalogError};
use algoram_core::{Graph, GraphError, PortChannel};
use algoram_interop::{Resolution, RouteError, RouteRegistry, RouteRequest};
use algoram_runtime::{
    DistributedExecutionTrace, ExecutionPlan, ExecutionStep, ExecutionTrace,
    ImplementationRegistry, RuntimeEndpoint, RuntimeLocationClass, TraceEntry, TraceStatus,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteConnectorHealth {
    Available,
    Unavailable,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureEvidence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_ref: Option<String>,
    pub entry: TraceEntry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactConnection {
    pub connection_id: String,
    pub source_block_id: String,
    pub target_block_id: String,
    pub channel: PortChannel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyImpact {
    pub failed_block_ids: Vec<String>,
    pub affected_block_ids: Vec<String>,
    pub traversed_connections: Vec<ImpactConnection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedRouteHealth {
    pub step_id: String,
    pub origin_block_ids: Vec<String>,
    pub connector_id: String,
    pub health: RouteConnectorHealth,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResilienceReport {
    pub reference_graph_id: String,
    pub failures: Vec<FailureEvidence>,
    pub impact: DependencyImpact,
    pub selected_route_health: Vec<SelectedRouteHealth>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlternativeRouteCandidate {
    pub step_id: String,
    pub origin_block_ids: Vec<String>,
    pub connection_id: String,
    pub source_contract: String,
    pub target_contract: String,
    pub connector_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustedImplementationCandidate {
    pub implementation_ref: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogImplementationCandidate {
    pub listing_id: String,
    pub supplier: String,
    pub implementation_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplementationRecoveryCandidates {
    pub logical_implementation_ref: String,
    pub trusted_local: Vec<TrustedImplementationCandidate>,
    pub catalog_only: Vec<CatalogImplementationCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeRecoveryCandidate {
    pub runtime_ref: String,
    pub class: RuntimeLocationClass,
    pub is_current: bool,
}

pub struct ResilienceAnalyzer;

impl ResilienceAnalyzer {
    pub fn analyze_execution(
        graph: &Graph,
        plan: &ExecutionPlan,
        trace: &ExecutionTrace,
        routes: &RouteRegistry,
    ) -> Result<ResilienceReport, ResilienceError> {
        validate_graph_and_plan(graph, plan)?;
        if trace.reference_graph_id != graph.id {
            return Err(ResilienceError::TraceGraphMismatch {
                graph_id: graph.id.clone(),
                trace_graph_id: trace.reference_graph_id.clone(),
            });
        }

        let failures = trace
            .entries
            .iter()
            .filter(|entry| entry.status == TraceStatus::Failed)
            .cloned()
            .map(|entry| FailureEvidence {
                runtime_ref: None,
                entry,
            })
            .collect::<Vec<_>>();

        Self::build_report(graph, plan, failures, routes)
    }

    pub fn analyze_distributed(
        graph: &Graph,
        plan: &ExecutionPlan,
        trace: &DistributedExecutionTrace,
        routes: &RouteRegistry,
    ) -> Result<ResilienceReport, ResilienceError> {
        validate_graph_and_plan(graph, plan)?;
        if trace.reference_graph_id != graph.id {
            return Err(ResilienceError::TraceGraphMismatch {
                graph_id: graph.id.clone(),
                trace_graph_id: trace.reference_graph_id.clone(),
            });
        }

        let failures = trace
            .entries
            .iter()
            .filter(|entry| entry.entry.status == TraceStatus::Failed)
            .map(|entry| FailureEvidence {
                runtime_ref: Some(entry.runtime_ref.clone()),
                entry: entry.entry.clone(),
            })
            .collect::<Vec<_>>();

        Self::build_report(graph, plan, failures, routes)
    }

    pub fn alternative_route_candidates(
        graph: &Graph,
        plan: &ExecutionPlan,
        routes: &RouteRegistry,
    ) -> Result<Vec<AlternativeRouteCandidate>, ResilienceError> {
        validate_graph_and_plan(graph, plan)?;

        let unhealthy_step_ids = selected_route_health(plan, routes)
            .into_iter()
            .filter(|health| health.health != RouteConnectorHealth::Available)
            .map(|health| health.step_id)
            .collect::<BTreeSet<_>>();

        let mut candidates = BTreeMap::<(String, String), AlternativeRouteCandidate>::new();

        for step in &plan.steps {
            if !unhealthy_step_ids.contains(&step.id) {
                continue;
            }

            for origin_block_id in &step.origin_block_ids {
                let mut incoming = graph
                    .connections
                    .iter()
                    .filter(|connection| connection.target.block_id == *origin_block_id)
                    .collect::<Vec<_>>();
                incoming.sort_by(|left, right| left.id.cmp(&right.id));

                for connection in incoming {
                    let source_port = find_graph_port(
                        graph,
                        &connection.id,
                        &connection.source.block_id,
                        &connection.source.port_id,
                        true,
                    )?;
                    let target_port = find_graph_port(
                        graph,
                        &connection.id,
                        &connection.target.block_id,
                        &connection.target.port_id,
                        false,
                    )?;

                    if source_port.channel != PortChannel::Data
                        || target_port.channel != PortChannel::Data
                    {
                        continue;
                    }

                    let Some(source_contract) = source_port
                        .contract
                        .as_ref()
                        .and_then(|value| value.as_str())
                    else {
                        continue;
                    };
                    let Some(target_contract) = target_port
                        .contract
                        .as_ref()
                        .and_then(|value| value.as_str())
                    else {
                        continue;
                    };
                    if source_contract == target_contract {
                        continue;
                    }

                    let resolution = routes
                        .resolve(&RouteRequest::automatic(source_contract, target_contract))
                        .map_err(ResilienceError::Route)?;
                    let Resolution::Resolved { route, .. } = resolution else {
                        continue;
                    };
                    if route.steps.is_empty() {
                        continue;
                    }

                    candidates.insert(
                        (step.id.clone(), connection.id.clone()),
                        AlternativeRouteCandidate {
                            step_id: step.id.clone(),
                            origin_block_ids: step.origin_block_ids.clone(),
                            connection_id: connection.id.clone(),
                            source_contract: source_contract.to_owned(),
                            target_contract: target_contract.to_owned(),
                            connector_ids: route
                                .connector_ids()
                                .into_iter()
                                .map(str::to_owned)
                                .collect(),
                        },
                    );
                }
            }
        }

        Ok(candidates.into_values().collect())
    }

    pub fn implementation_recovery_candidates(
        logical_implementation_ref: &str,
        implementations: &ImplementationRegistry,
        catalog: &CapabilityCatalog,
    ) -> Result<ImplementationRecoveryCandidates, ResilienceError> {
        let choice = implementations
            .choice(logical_implementation_ref)
            .ok_or_else(|| ResilienceError::MissingImplementationChoice {
                logical_implementation_ref: logical_implementation_ref.to_owned(),
            })?;

        let trusted_set = choice
            .candidates
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();

        let trusted_local = choice
            .candidates
            .iter()
            .map(|implementation_ref| TrustedImplementationCandidate {
                implementation_ref: implementation_ref.clone(),
                is_default: implementation_ref == &choice.default_ref,
            })
            .collect::<Vec<_>>();

        let catalog_only = catalog
            .search_logical_implementations(logical_implementation_ref)
            .map_err(ResilienceError::Catalog)?
            .into_iter()
            .filter_map(|listing| match &listing.asset {
                CatalogAsset::Implementation {
                    implementation_ref, ..
                } if !trusted_set.contains(implementation_ref.as_str()) => {
                    Some(CatalogImplementationCandidate {
                        listing_id: listing.listing_id.clone(),
                        supplier: listing.supplier.clone(),
                        implementation_ref: implementation_ref.clone(),
                    })
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        Ok(ImplementationRecoveryCandidates {
            logical_implementation_ref: logical_implementation_ref.to_owned(),
            trusted_local,
            catalog_only,
        })
    }

    pub fn runtime_recovery_candidates(
        step: &ExecutionStep,
        current_runtime_ref: &str,
        endpoints: &[RuntimeEndpoint],
    ) -> Vec<RuntimeRecoveryCandidate> {
        let mut candidates = endpoints
            .iter()
            .filter(|endpoint| endpoint.supports(&step.implementation_ref))
            .map(|endpoint| RuntimeRecoveryCandidate {
                runtime_ref: endpoint.runtime_ref.clone(),
                class: endpoint.class,
                is_current: endpoint.runtime_ref == current_runtime_ref,
            })
            .collect::<Vec<_>>();

        candidates.sort_by(|left, right| left.runtime_ref.cmp(&right.runtime_ref));
        candidates
    }

    fn build_report(
        graph: &Graph,
        plan: &ExecutionPlan,
        mut failures: Vec<FailureEvidence>,
        routes: &RouteRegistry,
    ) -> Result<ResilienceReport, ResilienceError> {
        if failures.is_empty() {
            return Err(ResilienceError::NoObservedFailure);
        }

        failures.sort_by(|left, right| {
            left.entry
                .step_id
                .cmp(&right.entry.step_id)
                .then(left.runtime_ref.cmp(&right.runtime_ref))
        });

        let step_by_id = plan
            .steps
            .iter()
            .map(|step| (step.id.as_str(), step))
            .collect::<BTreeMap<_, _>>();

        let graph_block_ids = graph
            .blocks
            .iter()
            .map(|block| block.id.as_str())
            .collect::<BTreeSet<_>>();

        let mut failed_block_ids = BTreeSet::<String>::new();

        for failure in &failures {
            let step = step_by_id
                .get(failure.entry.step_id.as_str())
                .ok_or_else(|| ResilienceError::UnknownTraceStep {
                    step_id: failure.entry.step_id.clone(),
                })?;

            validate_failure_attribution(&failure.entry, step)?;

            for block_id in &failure.entry.origin_block_ids {
                if !graph_block_ids.contains(block_id.as_str()) {
                    return Err(ResilienceError::UnknownOriginBlock {
                        step_id: failure.entry.step_id.clone(),
                        block_id: block_id.clone(),
                    });
                }
                failed_block_ids.insert(block_id.clone());
            }
        }

        let impact = dependency_impact(graph, failed_block_ids)?;
        let selected_route_health = selected_route_health(plan, routes);

        Ok(ResilienceReport {
            reference_graph_id: graph.id.clone(),
            failures,
            impact,
            selected_route_health,
        })
    }
}

fn validate_graph_and_plan(graph: &Graph, plan: &ExecutionPlan) -> Result<(), ResilienceError> {
    graph.validate().map_err(ResilienceError::Graph)?;
    if plan.reference_graph_id != graph.id {
        return Err(ResilienceError::PlanGraphMismatch {
            graph_id: graph.id.clone(),
            plan_graph_id: plan.reference_graph_id.clone(),
        });
    }
    Ok(())
}

fn find_graph_port<'a>(
    graph: &'a Graph,
    connection_id: &str,
    block_id: &str,
    port_id: &str,
    source: bool,
) -> Result<&'a algoram_core::Port, ResilienceError> {
    graph
        .blocks
        .iter()
        .find(|block| block.id == block_id)
        .and_then(|block| block.ports.iter().find(|port| port.id == port_id))
        .ok_or_else(|| {
            if source {
                ResilienceError::MissingConnectionSourcePort {
                    connection_id: connection_id.to_owned(),
                    block_id: block_id.to_owned(),
                    port_id: port_id.to_owned(),
                }
            } else {
                ResilienceError::MissingConnectionTargetPort {
                    connection_id: connection_id.to_owned(),
                    block_id: block_id.to_owned(),
                    port_id: port_id.to_owned(),
                }
            }
        })
}

fn validate_failure_attribution(
    entry: &TraceEntry,
    step: &algoram_runtime::ExecutionStep,
) -> Result<(), ResilienceError> {
    if entry.implementation_ref != step.implementation_ref {
        return Err(ResilienceError::TracePlanMismatch {
            step_id: entry.step_id.clone(),
            field: "implementation_ref",
        });
    }
    if entry.origin_block_ids != step.origin_block_ids {
        return Err(ResilienceError::TracePlanMismatch {
            step_id: entry.step_id.clone(),
            field: "origin_block_ids",
        });
    }
    if entry.source_anchors != step.source_anchors {
        return Err(ResilienceError::TracePlanMismatch {
            step_id: entry.step_id.clone(),
            field: "source_anchors",
        });
    }
    if entry.route_connector_ids != step.route_connector_ids {
        return Err(ResilienceError::TracePlanMismatch {
            step_id: entry.step_id.clone(),
            field: "route_connector_ids",
        });
    }
    Ok(())
}

fn dependency_impact(
    graph: &Graph,
    failed_block_ids: BTreeSet<String>,
) -> Result<DependencyImpact, ResilienceError> {
    let mut seen = failed_block_ids.clone();
    let mut frontier = failed_block_ids.clone();
    let mut affected = BTreeSet::<String>::new();
    let mut connections = BTreeMap::<String, ImpactConnection>::new();

    while let Some(source_block_id) = frontier.iter().next().cloned() {
        frontier.remove(&source_block_id);

        let mut outgoing = graph
            .connections
            .iter()
            .filter(|connection| connection.source.block_id == source_block_id)
            .collect::<Vec<_>>();
        outgoing.sort_by(|left, right| left.id.cmp(&right.id));

        for connection in outgoing {
            let channel = graph
                .blocks
                .iter()
                .find(|block| block.id == connection.source.block_id)
                .and_then(|block| {
                    block
                        .ports
                        .iter()
                        .find(|port| port.id == connection.source.port_id)
                })
                .map(|port| port.channel.clone())
                .ok_or_else(|| ResilienceError::MissingConnectionSourcePort {
                    connection_id: connection.id.clone(),
                    block_id: connection.source.block_id.clone(),
                    port_id: connection.source.port_id.clone(),
                })?;

            connections
                .entry(connection.id.clone())
                .or_insert_with(|| ImpactConnection {
                    connection_id: connection.id.clone(),
                    source_block_id: connection.source.block_id.clone(),
                    target_block_id: connection.target.block_id.clone(),
                    channel,
                });

            if seen.insert(connection.target.block_id.clone()) {
                affected.insert(connection.target.block_id.clone());
                frontier.insert(connection.target.block_id.clone());
            }
        }
    }

    Ok(DependencyImpact {
        failed_block_ids: failed_block_ids.into_iter().collect(),
        affected_block_ids: affected.into_iter().collect(),
        traversed_connections: connections.into_values().collect(),
    })
}

fn selected_route_health(plan: &ExecutionPlan, routes: &RouteRegistry) -> Vec<SelectedRouteHealth> {
    let mut report = Vec::new();

    for step in &plan.steps {
        for connector_id in &step.route_connector_ids {
            let health = match routes
                .connectors()
                .iter()
                .find(|connector| connector.id == *connector_id)
            {
                Some(connector) if connector.available => RouteConnectorHealth::Available,
                Some(_) => RouteConnectorHealth::Unavailable,
                None => RouteConnectorHealth::Unknown,
            };

            report.push(SelectedRouteHealth {
                step_id: step.id.clone(),
                origin_block_ids: step.origin_block_ids.clone(),
                connector_id: connector_id.clone(),
                health,
            });
        }
    }

    report.sort_by(|left, right| {
        left.step_id
            .cmp(&right.step_id)
            .then(left.connector_id.cmp(&right.connector_id))
    });
    report
}

#[derive(Debug)]
pub enum ResilienceError {
    Graph(GraphError),
    Catalog(CatalogError),
    MissingImplementationChoice {
        logical_implementation_ref: String,
    },
    PlanGraphMismatch {
        graph_id: String,
        plan_graph_id: String,
    },
    TraceGraphMismatch {
        graph_id: String,
        trace_graph_id: String,
    },
    NoObservedFailure,
    UnknownTraceStep {
        step_id: String,
    },
    TracePlanMismatch {
        step_id: String,
        field: &'static str,
    },
    UnknownOriginBlock {
        step_id: String,
        block_id: String,
    },
    Route(RouteError),
    MissingConnectionSourcePort {
        connection_id: String,
        block_id: String,
        port_id: String,
    },
    MissingConnectionTargetPort {
        connection_id: String,
        block_id: String,
        port_id: String,
    },
}

impl fmt::Display for ResilienceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Graph(error) => write!(f, "invalid reference graph: {error}"),
            Self::Catalog(error) => write!(f, "invalid capability catalog: {error}"),
            Self::MissingImplementationChoice {
                logical_implementation_ref,
            } => write!(
                f,
                "no trusted local implementation choice exists for logical ref '{logical_implementation_ref}'"
            ),
            Self::PlanGraphMismatch {
                graph_id,
                plan_graph_id,
            } => write!(
                f,
                "execution plan references graph '{plan_graph_id}', expected '{graph_id}'"
            ),
            Self::TraceGraphMismatch {
                graph_id,
                trace_graph_id,
            } => write!(
                f,
                "execution trace references graph '{trace_graph_id}', expected '{graph_id}'"
            ),
            Self::NoObservedFailure => write!(f, "execution evidence contains no failed step"),
            Self::UnknownTraceStep { step_id } => {
                write!(
                    f,
                    "failed trace step '{step_id}' is not present in the execution plan"
                )
            }
            Self::TracePlanMismatch { step_id, field } => write!(
                f,
                "failed trace step '{step_id}' does not match plan attribution field '{field}'"
            ),
            Self::UnknownOriginBlock { step_id, block_id } => write!(
                f,
                "failed trace step '{step_id}' references unknown origin block '{block_id}'"
            ),
            Self::Route(error) => write!(f, "route candidate resolution failed: {error}"),
            Self::MissingConnectionSourcePort {
                connection_id,
                block_id,
                port_id,
            } => write!(
                f,
                "connection '{connection_id}' references missing source port '{block_id}.{port_id}'"
            ),
            Self::MissingConnectionTargetPort {
                connection_id,
                block_id,
                port_id,
            } => write!(
                f,
                "connection '{connection_id}' references missing target port '{block_id}.{port_id}'"
            ),
        }
    }
}

impl Error for ResilienceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Graph(error) => Some(error),
            Self::Catalog(error) => Some(error),
            Self::Route(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use algoram_catalog::{
        CapabilityListing, CommercialAvailability, Inspectability, MarketplaceMetadata, Price,
        RatingSummary, SourceAvailability,
    };
    use algoram_core::{Block, Connection, Extensions, Port, PortDirection, PortRef, SourceAnchor};
    use algoram_interop::{Connector, ContractId};
    use algoram_runtime::{
        DistributedTraceEntry, PlacedExecutionPlan, Planner, ProcessAction, StepPlacement,
        TraceStatus,
    };
    use std::fs;

    fn data_port(id: &str, direction: PortDirection) -> Port {
        Port {
            id: id.to_owned(),
            direction,
            channel: PortChannel::Data,
            contract: None,
            extensions: Extensions::new(),
        }
    }

    fn block(id: &str) -> Block {
        Block {
            id: id.to_owned(),
            label: id.to_owned(),
            ports: vec![
                data_port("in", PortDirection::In),
                data_port("out", PortDirection::Out),
            ],
            internal_graph_ref: None,
            implementation_ref: Some(format!("impl:{id}")),
            definition_ref: None,
            source_anchor: None,
            extensions: Extensions::new(),
            diagnostics: Vec::new(),
        }
    }

    fn connection(id: &str, source: &str, target: &str) -> Connection {
        Connection {
            id: id.to_owned(),
            source: PortRef {
                block_id: source.to_owned(),
                port_id: "out".to_owned(),
            },
            target: PortRef {
                block_id: target.to_owned(),
                port_id: "in".to_owned(),
            },
            extensions: Extensions::new(),
        }
    }

    fn cyclic_graph() -> Graph {
        let mut graph = Graph::new("graph:resilience");
        graph.blocks.extend([block("a"), block("b"), block("c")]);
        graph.connections.extend([
            connection("a-b", "a", "b"),
            connection("b-c", "b", "c"),
            connection("c-a", "c", "a"),
        ]);
        graph
    }

    fn step(
        block_id: &str,
        route_connector_ids: impl IntoIterator<Item = impl Into<String>>,
        action: ProcessAction,
    ) -> ExecutionStep {
        ExecutionStep {
            id: format!("step:{block_id}"),
            implementation_ref: format!("impl:{block_id}"),
            action,
            origin_block_ids: vec![block_id.to_owned()],
            source_anchors: Vec::<SourceAnchor>::new(),
            route_connector_ids: route_connector_ids.into_iter().map(Into::into).collect(),
            argv_bindings: Vec::new(),
        }
    }

    fn plan_with_dangerous_action(marker: &std::path::Path) -> ExecutionPlan {
        ExecutionPlan {
            reference_graph_id: "graph:resilience".to_owned(),
            steps: vec![
                step(
                    "a",
                    [
                        "connector:available",
                        "connector:unavailable",
                        "connector:unknown",
                    ],
                    ProcessAction::new(
                        "sh",
                        ["-c".to_owned(), format!("touch {}", marker.display())],
                    ),
                ),
                step(
                    "b",
                    std::iter::empty::<&str>(),
                    ProcessAction::new("b", std::iter::empty::<&str>()),
                ),
                step(
                    "c",
                    std::iter::empty::<&str>(),
                    ProcessAction::new("c", std::iter::empty::<&str>()),
                ),
            ],
        }
    }

    fn failed_entry(plan: &ExecutionPlan) -> TraceEntry {
        let step = &plan.steps[0];
        TraceEntry {
            step_id: step.id.clone(),
            implementation_ref: step.implementation_ref.clone(),
            status: TraceStatus::Failed,
            origin_block_ids: step.origin_block_ids.clone(),
            source_anchors: step.source_anchors.clone(),
            route_connector_ids: step.route_connector_ids.clone(),
            exit_code: Some(9),
            stdout: String::new(),
            stderr: "provider unavailable".to_owned(),
        }
    }

    fn routes() -> RouteRegistry {
        let mut routes = RouteRegistry::new();
        routes
            .register(Connector::new(
                "connector:available",
                ContractId::from("a"),
                ContractId::from("b"),
                "impl:route-a",
            ))
            .unwrap();
        routes
            .register(
                Connector::new(
                    "connector:unavailable",
                    ContractId::from("a"),
                    ContractId::from("c"),
                    "impl:route-b",
                )
                .unavailable(),
            )
            .unwrap();
        routes
    }

    #[test]
    fn local_failure_maps_to_cycle_safe_dependency_exposure_and_route_health() {
        let marker = std::env::temp_dir().join(format!(
            "algoram-resilience-must-not-execute-{}",
            std::process::id()
        ));
        let _ = fs::remove_file(&marker);

        let graph = cyclic_graph();
        let graph_before = graph.clone();
        let plan = plan_with_dangerous_action(&marker);
        let plan_before = plan.clone();
        let trace = ExecutionTrace {
            reference_graph_id: graph.id.clone(),
            entries: vec![failed_entry(&plan)],
        };
        let route_registry = routes();
        let route_count = route_registry.connectors().len();

        let report =
            ResilienceAnalyzer::analyze_execution(&graph, &plan, &trace, &route_registry).unwrap();

        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].entry.stderr, "provider unavailable");
        assert_eq!(report.failures[0].entry.exit_code, Some(9));
        assert_eq!(report.impact.failed_block_ids, vec!["a"]);
        assert_eq!(report.impact.affected_block_ids, vec!["b", "c"]);
        assert_eq!(
            report
                .impact
                .traversed_connections
                .iter()
                .map(|connection| connection.connection_id.as_str())
                .collect::<Vec<_>>(),
            vec!["a-b", "b-c", "c-a"]
        );
        assert!(report
            .impact
            .traversed_connections
            .iter()
            .all(|connection| connection.channel == PortChannel::Data));
        assert_eq!(
            report
                .selected_route_health
                .iter()
                .map(|health| (health.connector_id.as_str(), health.health))
                .collect::<Vec<_>>(),
            vec![
                ("connector:available", RouteConnectorHealth::Available),
                ("connector:unavailable", RouteConnectorHealth::Unavailable),
                ("connector:unknown", RouteConnectorHealth::Unknown),
            ]
        );

        assert_eq!(graph, graph_before);
        assert_eq!(plan, plan_before);
        assert_eq!(route_registry.connectors().len(), route_count);
        assert!(!marker.exists());
    }

    #[test]
    fn distributed_failure_retains_runtime_ref_and_underlying_trace_entry() {
        let marker = std::env::temp_dir().join(format!(
            "algoram-resilience-distributed-must-not-execute-{}",
            std::process::id()
        ));
        let _ = fs::remove_file(&marker);

        let graph = cyclic_graph();
        let plan = plan_with_dangerous_action(&marker);
        let entry = failed_entry(&plan);
        let trace = DistributedExecutionTrace {
            reference_graph_id: graph.id.clone(),
            entries: vec![DistributedTraceEntry {
                runtime_ref: "runtime:agent-a".to_owned(),
                entry: entry.clone(),
            }],
            transfers: Vec::new(),
        };

        let report =
            ResilienceAnalyzer::analyze_distributed(&graph, &plan, &trace, &routes()).unwrap();

        assert_eq!(
            report.failures[0].runtime_ref.as_deref(),
            Some("runtime:agent-a")
        );
        assert_eq!(report.failures[0].entry, entry);
        assert!(!marker.exists());
    }

    #[test]
    fn forged_failure_attribution_is_rejected() {
        let marker = std::env::temp_dir().join(format!(
            "algoram-resilience-forged-must-not-execute-{}",
            std::process::id()
        ));
        let _ = fs::remove_file(&marker);

        let graph = cyclic_graph();
        let plan = plan_with_dangerous_action(&marker);
        let mut forged = failed_entry(&plan);
        forged.implementation_ref = "impl:forged".to_owned();
        let trace = ExecutionTrace {
            reference_graph_id: graph.id.clone(),
            entries: vec![forged],
        };

        let error =
            ResilienceAnalyzer::analyze_execution(&graph, &plan, &trace, &routes()).unwrap_err();

        assert!(matches!(
            error,
            ResilienceError::TracePlanMismatch {
                field: "implementation_ref",
                ..
            }
        ));
        assert!(!marker.exists());
    }

    #[test]
    fn failure_free_trace_is_not_misreported_as_recovery_evidence() {
        let marker = std::env::temp_dir().join(format!(
            "algoram-resilience-success-must-not-execute-{}",
            std::process::id()
        ));
        let _ = fs::remove_file(&marker);

        let graph = cyclic_graph();
        let plan = plan_with_dangerous_action(&marker);
        let step = &plan.steps[0];
        let trace = ExecutionTrace {
            reference_graph_id: graph.id.clone(),
            entries: vec![TraceEntry {
                step_id: step.id.clone(),
                implementation_ref: step.implementation_ref.clone(),
                status: TraceStatus::Succeeded,
                origin_block_ids: step.origin_block_ids.clone(),
                source_anchors: step.source_anchors.clone(),
                route_connector_ids: step.route_connector_ids.clone(),
                exit_code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
            }],
        };

        let error =
            ResilienceAnalyzer::analyze_execution(&graph, &plan, &trace, &routes()).unwrap_err();

        assert!(matches!(error, ResilienceError::NoObservedFailure));
        assert!(!marker.exists());
    }

    #[test]
    fn unhealthy_selected_route_surfaces_current_reachable_alternative_without_switching() {
        let marker = std::env::temp_dir().join(format!(
            "algoram-resilience-route-candidate-must-not-execute-{}",
            std::process::id()
        ));
        let _ = fs::remove_file(&marker);

        let mut graph = Graph::new("graph:route-candidate");
        let mut source = block("route-source");
        source.implementation_ref = None;
        source.ports.clear();
        source.ports.push(Port {
            id: "out".to_owned(),
            direction: PortDirection::Out,
            channel: PortChannel::Data,
            contract: Some("contract:a".into()),
            extensions: Extensions::new(),
        });
        let mut target = block("route-target");
        target.ports.clear();
        target.ports.push(Port {
            id: "in".to_owned(),
            direction: PortDirection::In,
            channel: PortChannel::Data,
            contract: Some("contract:c".into()),
            extensions: Extensions::new(),
        });
        graph.blocks.extend([source, target]);
        graph.connections.push(Connection {
            id: "data:route-candidate".to_owned(),
            source: PortRef {
                block_id: "route-source".to_owned(),
                port_id: "out".to_owned(),
            },
            target: PortRef {
                block_id: "route-target".to_owned(),
                port_id: "in".to_owned(),
            },
            extensions: Extensions::new(),
        });

        let plan = ExecutionPlan {
            reference_graph_id: graph.id.clone(),
            steps: vec![ExecutionStep {
                id: "step:route-target".to_owned(),
                implementation_ref: "impl:route-target".to_owned(),
                action: ProcessAction::new(
                    "sh",
                    ["-c".to_owned(), format!("touch {}", marker.display())],
                ),
                origin_block_ids: vec!["route-target".to_owned()],
                source_anchors: Vec::new(),
                route_connector_ids: vec!["primary".to_owned()],
                argv_bindings: Vec::new(),
            }],
        };
        let graph_before = graph.clone();
        let plan_before = plan.clone();

        let mut routes = RouteRegistry::new();
        routes
            .register(
                Connector::new(
                    "primary",
                    ContractId::from("contract:a"),
                    ContractId::from("contract:c"),
                    "impl:primary",
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
        let connector_count = routes.connectors().len();

        let candidates =
            ResilienceAnalyzer::alternative_route_candidates(&graph, &plan, &routes).unwrap();

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].step_id, "step:route-target");
        assert_eq!(candidates[0].connection_id, "data:route-candidate");
        assert_eq!(candidates[0].source_contract, "contract:a");
        assert_eq!(candidates[0].target_contract, "contract:c");
        assert_eq!(candidates[0].connector_ids, vec!["a-1", "a-2"]);
        assert_eq!(graph, graph_before);
        assert_eq!(plan, plan_before);
        assert_eq!(routes.connectors().len(), connector_count);
        assert!(!marker.exists());
    }

    fn provider_listing(
        listing_id: &str,
        supplier: &str,
        implementation_ref: &str,
        amount_minor: u64,
        rating_score: u16,
    ) -> CapabilityListing {
        CapabilityListing {
            listing_id: listing_id.to_owned(),
            title: format!("{supplier} provider"),
            supplier: supplier.to_owned(),
            source: format!("catalog://{supplier}"),
            version: "1".to_owned(),
            last_updated: "2026-10-06".to_owned(),
            asset: CatalogAsset::Implementation {
                implementation_ref: implementation_ref.to_owned(),
                logical_implementation_ref: Some("logical:provider".to_owned()),
            },
            supported_ports: Vec::new(),
            marketplace: MarketplaceMetadata {
                commercial_availability: CommercialAvailability::Paid,
                source_availability: SourceAvailability::Closed,
                price: Some(Price {
                    amount_minor,
                    currency: "JPY".to_owned(),
                }),
                rating: Some(RatingSummary {
                    score_millis: rating_score,
                    max_score_millis: 5000,
                    review_count: 10,
                }),
                reputation: None,
                inspectability: Inspectability::MetadataOnly,
                support_statement: None,
                warranty_statement: None,
            },
        }
    }

    fn provider_graph() -> Graph {
        let mut graph = Graph::new("graph:provider-recovery");
        let mut provider = block("provider");
        provider.implementation_ref = Some("logical:provider".to_owned());
        graph.blocks.push(provider);
        graph
    }

    fn provider_registry(default_ref: &str, marker: &std::path::Path) -> ImplementationRegistry {
        let mut registry = ImplementationRegistry::new();
        registry
            .register(
                "impl:trusted-a",
                ProcessAction::new(
                    "sh",
                    ["-c".to_owned(), format!("touch {}", marker.display())],
                ),
            )
            .unwrap();
        registry
            .register(
                "impl:trusted-b",
                ProcessAction::new(
                    "sh",
                    ["-c".to_owned(), format!("touch {}", marker.display())],
                ),
            )
            .unwrap();
        registry
            .register_choice(
                "logical:provider",
                ["impl:trusted-a", "impl:trusted-b"],
                default_ref,
            )
            .unwrap();
        registry
    }

    #[test]
    fn provider_candidates_separate_trusted_local_from_catalog_only_without_ranking_execution() {
        let marker = std::env::temp_dir().join(format!(
            "algoram-resilience-provider-candidates-must-not-execute-{}",
            std::process::id()
        ));
        let _ = fs::remove_file(&marker);

        let registry = provider_registry("impl:trusted-a", &marker);
        let catalog = CapabilityCatalog::new([
            provider_listing(
                "listing:z-trusted",
                "trusted supplier",
                "impl:trusted-b",
                1,
                5000,
            ),
            provider_listing(
                "listing:a-catalog-only",
                "catalog supplier",
                "impl:catalog-only",
                999_999,
                100,
            ),
        ]);
        let catalog_before = catalog.clone();

        let candidates = ResilienceAnalyzer::implementation_recovery_candidates(
            "logical:provider",
            &registry,
            &catalog,
        )
        .unwrap();

        assert_eq!(
            candidates
                .trusted_local
                .iter()
                .map(|candidate| (candidate.implementation_ref.as_str(), candidate.is_default))
                .collect::<Vec<_>>(),
            vec![("impl:trusted-a", true), ("impl:trusted-b", false)]
        );
        assert_eq!(candidates.catalog_only.len(), 1);
        assert_eq!(
            candidates.catalog_only[0].implementation_ref,
            "impl:catalog-only"
        );
        assert_eq!(
            candidates.catalog_only[0].listing_id,
            "listing:a-catalog-only"
        );
        assert_eq!(catalog, catalog_before);
        assert!(!marker.exists());
    }

    #[test]
    fn explicit_provider_switch_and_runtime_replacement_keep_graph_unchanged() {
        let marker = std::env::temp_dir().join(format!(
            "algoram-resilience-manual-switch-must-not-execute-{}",
            std::process::id()
        ));
        let _ = fs::remove_file(&marker);

        let graph = provider_graph();
        let graph_before = graph.clone();

        let plan_a = Planner::lower(
            &graph,
            &provider_registry("impl:trusted-a", &marker),
            &RouteRegistry::new(),
        )
        .unwrap();
        let plan_b = Planner::lower(
            &graph,
            &provider_registry("impl:trusted-b", &marker),
            &RouteRegistry::new(),
        )
        .unwrap();

        assert_eq!(graph, graph_before);
        assert_eq!(plan_a.steps[0].implementation_ref, "impl:trusted-a");
        assert_eq!(plan_b.steps[0].implementation_ref, "impl:trusted-b");

        let serialized = serde_json::to_string(&plan_b).unwrap();
        let replayed: ExecutionPlan = serde_json::from_str(&serialized).unwrap();
        assert_eq!(replayed.steps[0].implementation_ref, "impl:trusted-b");

        let endpoints = vec![
            RuntimeEndpoint::new(
                "runtime:agent-current",
                RuntimeLocationClass::RuntimeAgent,
                ["impl:trusted-b"],
            ),
            RuntimeEndpoint::new(
                "runtime:local-alternate",
                RuntimeLocationClass::LocalProcess,
                ["impl:trusted-b"],
            ),
            RuntimeEndpoint::new(
                "runtime:unrelated",
                RuntimeLocationClass::LocalProcess,
                ["impl:trusted-a"],
            ),
        ];

        let runtime_candidates = ResilienceAnalyzer::runtime_recovery_candidates(
            &plan_b.steps[0],
            "runtime:agent-current",
            &endpoints,
        );
        assert_eq!(
            runtime_candidates
                .iter()
                .map(|candidate| (candidate.runtime_ref.as_str(), candidate.is_current))
                .collect::<Vec<_>>(),
            vec![
                ("runtime:agent-current", true),
                ("runtime:local-alternate", false),
            ]
        );

        let current = PlacedExecutionPlan::new(
            plan_b.clone(),
            [StepPlacement::new(
                plan_b.steps[0].id.clone(),
                "runtime:agent-current",
            )],
        );
        current.validate(&endpoints).unwrap();

        let recovered = PlacedExecutionPlan::new(
            plan_b.clone(),
            [StepPlacement::new(
                plan_b.steps[0].id.clone(),
                "runtime:local-alternate",
            )],
        );
        recovered.validate(&endpoints).unwrap();

        assert_eq!(graph, graph_before);
        assert_eq!(recovered.plan, plan_b);
        assert_eq!(
            recovered.runtime_for_step(&plan_b.steps[0].id),
            Some("runtime:local-alternate")
        );
        assert!(!marker.exists());
    }

    fn provider_listing(
        listing_id: &str,
        supplier: &str,
        implementation_ref: &str,
        amount_minor: u64,
        score_millis: u16,
    ) -> CapabilityListing {
        CapabilityListing {
            listing_id: listing_id.to_owned(),
            title: format!("{supplier} provider"),
            supplier: supplier.to_owned(),
            source: format!("https://example.invalid/{supplier}"),
            version: "1.0.0".to_owned(),
            last_updated: "2026-10-06".to_owned(),
            asset: CatalogAsset::Implementation {
                implementation_ref: implementation_ref.to_owned(),
                logical_implementation_ref: Some("logical:provider".to_owned()),
            },
            supported_ports: Vec::new(),
            marketplace: MarketplaceMetadata {
                commercial_availability: CommercialAvailability::Paid,
                source_availability: SourceAvailability::Closed,
                price: Some(Price {
                    amount_minor,
                    currency: "JPY".to_owned(),
                }),
                rating: Some(RatingSummary {
                    score_millis,
                    max_score_millis: 5000,
                    review_count: 10,
                }),
                reputation: None,
                inspectability: Inspectability::MetadataOnly,
                support_statement: None,
                warranty_statement: None,
            },
        }
    }

    fn provider_graph() -> Graph {
        let mut graph = Graph::new("graph:provider-recovery");
        graph.blocks.push(Block {
            id: "block:provider".to_owned(),
            label: "provider".to_owned(),
            ports: Vec::new(),
            internal_graph_ref: None,
            implementation_ref: Some("logical:provider".to_owned()),
            definition_ref: None,
            source_anchor: None,
            extensions: Extensions::new(),
            diagnostics: Vec::new(),
        });
        graph
    }

    fn provider_registry(
        default_ref: &str,
        marker: &std::path::Path,
    ) -> ImplementationRegistry {
        let mut registry = ImplementationRegistry::new();
        registry
            .register(
                "impl:provider-a",
                ProcessAction::new(
                    "sh",
                    ["-c".to_owned(), format!("touch {}", marker.display())],
                ),
            )
            .unwrap();
        registry
            .register(
                "impl:provider-b",
                ProcessAction::new(
                    "sh",
                    ["-c".to_owned(), format!("touch {}", marker.display())],
                ),
            )
            .unwrap();
        registry
            .register_choice(
                "logical:provider",
                ["impl:provider-a", "impl:provider-b"],
                default_ref,
            )
            .unwrap();
        registry
    }

    #[test]
    fn provider_recovery_separates_trusted_local_from_catalog_only_without_execution() {
        let marker = std::env::temp_dir().join(format!(
            "algoram-provider-candidate-must-not-execute-{}",
            std::process::id()
        ));
        let _ = fs::remove_file(&marker);

        let registry = provider_registry("impl:provider-a", &marker);
        let catalog = CapabilityCatalog::new([
            provider_listing(
                "listing:a-expensive",
                "catalog-expensive",
                "impl:catalog-expensive",
                999_900,
                100,
            ),
            provider_listing(
                "listing:trusted-b",
                "trusted-b-listing",
                "impl:provider-b",
                1,
                5000,
            ),
            provider_listing(
                "listing:z-cheap",
                "catalog-cheap",
                "impl:catalog-cheap",
                1,
                5000,
            ),
        ]);

        let candidates = ResilienceAnalyzer::implementation_recovery_candidates(
            "logical:provider",
            &registry,
            &catalog,
        )
        .unwrap();

        assert_eq!(candidates.logical_implementation_ref, "logical:provider");
        assert_eq!(
            candidates
                .trusted_local
                .iter()
                .map(|candidate| (
                    candidate.implementation_ref.as_str(),
                    candidate.is_default
                ))
                .collect::<Vec<_>>(),
            vec![
                ("impl:provider-a", true),
                ("impl:provider-b", false),
            ]
        );
        assert_eq!(
            candidates
                .catalog_only
                .iter()
                .map(|candidate| (
                    candidate.listing_id.as_str(),
                    candidate.implementation_ref.as_str()
                ))
                .collect::<Vec<_>>(),
            vec![
                ("listing:a-expensive", "impl:catalog-expensive"),
                ("listing:z-cheap", "impl:catalog-cheap"),
            ]
        );
        assert!(!candidates
            .catalog_only
            .iter()
            .any(|candidate| candidate.implementation_ref == "impl:provider-b"));
        assert!(!marker.exists());
    }

    #[test]
    fn explicit_provider_switch_reuses_existing_choice_and_keeps_graph_unchanged() {
        let marker = std::env::temp_dir().join(format!(
            "algoram-provider-switch-must-not-execute-{}",
            std::process::id()
        ));
        let _ = fs::remove_file(&marker);

        let graph = provider_graph();
        let graph_before = graph.clone();
        let registry = provider_registry("impl:provider-b", &marker);
        let plan = Planner::lower(&graph, &registry, &RouteRegistry::new()).unwrap();

        assert_eq!(graph, graph_before);
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].implementation_ref, "impl:provider-b");

        let serialized = serde_json::to_string(&plan).unwrap();
        let replayed: ExecutionPlan = serde_json::from_str(&serialized).unwrap();
        assert_eq!(replayed.steps[0].implementation_ref, "impl:provider-b");
        assert!(!marker.exists());
    }

    #[test]
    fn runtime_recovery_candidates_are_deterministic_and_manual_replacement_validates() {
        let marker = std::env::temp_dir().join(format!(
            "algoram-runtime-candidate-must-not-execute-{}",
            std::process::id()
        ));
        let _ = fs::remove_file(&marker);

        let graph = provider_graph();
        let graph_before = graph.clone();
        let registry = provider_registry("impl:provider-b", &marker);
        let plan = Planner::lower(&graph, &registry, &RouteRegistry::new()).unwrap();
        let plan_before = plan.clone();
        let step = &plan.steps[0];

        let endpoints = vec![
            RuntimeEndpoint::new(
                "runtime:local-b",
                RuntimeLocationClass::LocalProcess,
                ["impl:provider-b"],
            ),
            RuntimeEndpoint::new(
                "runtime:agent-current",
                RuntimeLocationClass::RuntimeAgent,
                ["impl:provider-b"],
            ),
            RuntimeEndpoint::new(
                "runtime:agent-alt",
                RuntimeLocationClass::RuntimeAgent,
                ["impl:provider-b"],
            ),
            RuntimeEndpoint::new(
                "runtime:unsupported",
                RuntimeLocationClass::RuntimeAgent,
                ["impl:other"],
            ),
        ];

        let candidates = ResilienceAnalyzer::runtime_recovery_candidates(
            step,
            "runtime:agent-current",
            &endpoints,
        );
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| (
                    candidate.runtime_ref.as_str(),
                    candidate.is_current,
                    candidate.class
                ))
                .collect::<Vec<_>>(),
            vec![
                ("runtime:agent-alt", false, RuntimeLocationClass::RuntimeAgent),
                (
                    "runtime:agent-current",
                    true,
                    RuntimeLocationClass::RuntimeAgent
                ),
                ("runtime:local-b", false, RuntimeLocationClass::LocalProcess),
            ]
        );
        assert!(!candidates
            .iter()
            .any(|candidate| candidate.runtime_ref == "runtime:unsupported"));

        let replacement = PlacedExecutionPlan::new(
            plan.clone(),
            [StepPlacement::new(
                step.id.clone(),
                "runtime:agent-alt",
            )],
        );
        replacement.validate(&endpoints).unwrap();

        assert_eq!(replacement.plan, plan);
        assert_eq!(
            replacement.runtime_for_step(&step.id),
            Some("runtime:agent-alt")
        );
        assert_eq!(graph, graph_before);
        assert_eq!(plan, plan_before);
        assert!(!marker.exists());
    }

    #[test]
    fn remote_failure_does_not_silently_run_local_recovery_candidate() {
        let marker = std::env::temp_dir().join(format!(
            "algoram-runtime-no-fallback-must-not-execute-{}",
            std::process::id()
        ));
        let _ = fs::remove_file(&marker);

        let graph = provider_graph();
        let registry = provider_registry("impl:provider-b", &marker);
        let plan = Planner::lower(&graph, &registry, &RouteRegistry::new()).unwrap();
        let step = &plan.steps[0];

        let endpoints = vec![
            RuntimeEndpoint::new(
                "runtime:agent-current",
                RuntimeLocationClass::RuntimeAgent,
                ["impl:provider-b"],
            ),
            RuntimeEndpoint::new(
                "runtime:local-recovery",
                RuntimeLocationClass::LocalProcess,
                ["impl:provider-b"],
            ),
        ];
        let placed = PlacedExecutionPlan::new(
            plan,
            [StepPlacement::new(
                step.id.clone(),
                "runtime:agent-current",
            )],
        );
        placed.validate(&endpoints).unwrap();

        let mut policy = ExecutionPolicy::new();
        policy.allow("impl:provider-b");

        let error = DistributedRuntime::execute_with_agent(
            &placed,
            &endpoints,
            &registry,
            &policy,
            &[],
            |runtime_ref, _request| {
                assert_eq!(runtime_ref, "runtime:agent-current");
                Err("agent offline".to_owned())
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            DistributedExecutionError::AgentUnavailable { runtime_ref, .. }
                if runtime_ref == "runtime:agent-current"
        ));
        assert!(!marker.exists());
    }

}
