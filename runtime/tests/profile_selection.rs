use algoram_core::{Block, Extensions, Graph};
use algoram_interop::RouteRegistry;
use algoram_runtime::{
    ExecutionPlan, ImplementationProfileStore, ImplementationRegistry, Planner, ProcessAction,
    ProcessRuntime,
};

fn block(id: &str, implementation_ref: &str) -> Block {
    Block {
        id: id.to_owned(),
        label: id.to_owned(),
        ports: Vec::new(),
        internal_graph_ref: None,
        implementation_ref: Some(implementation_ref.to_owned()),
        definition_ref: None,
        source_anchor: None,
        extensions: Extensions::new(),
        diagnostics: Vec::new(),
    }
}

fn graph(implementation_ref: &str) -> Graph {
    let mut graph = Graph::new("graph:profile-selection");
    graph
        .blocks
        .push(block("block:choice-target", implementation_ref));
    graph
}

fn implementations() -> ImplementationRegistry {
    let mut implementations = ImplementationRegistry::new();
    implementations
        .register(
            "impl:first",
            ProcessAction::new("python3", ["-c", "print('FIRST')"]),
        )
        .unwrap();
    implementations
        .register(
            "impl:second",
            ProcessAction::new("python3", ["-c", "print('SECOND')"]),
        )
        .unwrap();
    implementations
        .register_choice(
            "choice:greeting",
            ["impl:first", "impl:second"],
            "impl:first",
        )
        .unwrap();
    implementations
}

#[test]
fn default_planning_remains_unchanged_without_profile_context() {
    let graph = graph("choice:greeting");
    let implementations = implementations();

    let plan = Planner::lower(&graph, &implementations, &RouteRegistry::new()).unwrap();

    assert_eq!(plan.steps.len(), 1);
    assert_eq!(plan.steps[0].implementation_ref, "impl:first");
}

#[test]
fn stored_profile_selects_lowest_observed_candidate_for_environment() {
    let graph = graph("choice:greeting");
    let implementations = implementations();
    let mut profiles = ImplementationProfileStore::new();
    profiles.record("env:a", "impl:first", 200);
    profiles.record("env:a", "impl:second", 100);

    let plan = Planner::lower_with_profiles(
        &graph,
        &implementations,
        &RouteRegistry::new(),
        &profiles,
        "env:a",
    )
    .unwrap();

    assert_eq!(plan.steps[0].implementation_ref, "impl:second");
}

#[test]
fn profile_from_other_environment_does_not_affect_selection() {
    let graph = graph("choice:greeting");
    let implementations = implementations();
    let mut profiles = ImplementationProfileStore::new();
    profiles.record("env:other", "impl:first", 200);
    profiles.record("env:other", "impl:second", 100);

    let plan = Planner::lower_with_profiles(
        &graph,
        &implementations,
        &RouteRegistry::new(),
        &profiles,
        "env:target",
    )
    .unwrap();

    assert_eq!(plan.steps[0].implementation_ref, "impl:first");
}

#[test]
fn tied_profile_evidence_prefers_explicit_default() {
    let graph = graph("choice:greeting");
    let implementations = implementations();
    let mut profiles = ImplementationProfileStore::new();
    profiles.record("env:a", "impl:first", 100);
    profiles.record("env:a", "impl:second", 100);

    let plan = Planner::lower_with_profiles(
        &graph,
        &implementations,
        &RouteRegistry::new(),
        &profiles,
        "env:a",
    )
    .unwrap();

    assert_eq!(plan.steps[0].implementation_ref, "impl:first");
}

#[test]
fn same_graph_can_replan_to_different_concrete_implementation_from_stored_evidence() {
    let (first_serialized, second_serialized) = {
        let graph = graph("choice:greeting");
        let implementations = implementations();
        let routes = RouteRegistry::new();
        let mut profiles = ImplementationProfileStore::new();

        profiles.record("env:a", "impl:first", 200);
        profiles.record("env:a", "impl:second", 100);
        let second_plan =
            Planner::lower_with_profiles(&graph, &implementations, &routes, &profiles, "env:a")
                .unwrap();
        assert_eq!(second_plan.steps[0].implementation_ref, "impl:second");

        profiles.record("env:a", "impl:first", 50);
        let first_plan =
            Planner::lower_with_profiles(&graph, &implementations, &routes, &profiles, "env:a")
                .unwrap();
        assert_eq!(first_plan.steps[0].implementation_ref, "impl:first");

        (
            serde_json::to_string_pretty(&first_plan).unwrap(),
            serde_json::to_string_pretty(&second_plan).unwrap(),
        )
    };

    let first_plan: ExecutionPlan = serde_json::from_str(&first_serialized).unwrap();
    let second_plan: ExecutionPlan = serde_json::from_str(&second_serialized).unwrap();

    let first_trace = ProcessRuntime::execute(&first_plan);
    let second_trace = ProcessRuntime::execute(&second_plan);

    assert!(first_trace.succeeded());
    assert!(second_trace.succeeded());
    assert_eq!(first_trace.entries[0].stdout.trim(), "FIRST");
    assert_eq!(second_trace.entries[0].stdout.trim(), "SECOND");
}

#[test]
fn direct_concrete_ref_bypasses_profile_selection() {
    let graph = graph("impl:first");
    let implementations = implementations();
    let mut profiles = ImplementationProfileStore::new();
    profiles.record("env:a", "impl:first", 500);
    profiles.record("env:a", "impl:second", 1);

    let plan = Planner::lower_with_profiles(
        &graph,
        &implementations,
        &RouteRegistry::new(),
        &profiles,
        "env:a",
    )
    .unwrap();

    assert_eq!(plan.steps[0].implementation_ref, "impl:first");
}

#[test]
fn recording_new_evidence_replaces_previous_observation() {
    let mut profiles = ImplementationProfileStore::new();

    profiles.record("env:a", "impl:first", 200);
    assert_eq!(profiles.observed_ns("env:a", "impl:first"), Some(200));

    profiles.record("env:a", "impl:first", 75);
    assert_eq!(profiles.observed_ns("env:a", "impl:first"), Some(75));
}
