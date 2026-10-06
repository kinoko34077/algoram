use algoram_core::{Block, Extensions, Graph};
use algoram_interop::RouteRegistry;
use algoram_runtime::{
    ExecutionPlan, ExecutionStep, ImplementationProfileStore, ImplementationRegistry, Planner,
    ProcessAction, ProcessRuntime, TraceStatus,
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
    let mut graph = Graph::new("graph:trace-implementation");
    graph
        .blocks
        .push(block("block:trace-target", implementation_ref));
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
fn profile_selected_replay_trace_exposes_final_concrete_implementation() {
    let serialized = {
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
        serde_json::to_string_pretty(&plan).unwrap()
    };

    let plan: ExecutionPlan = serde_json::from_str(&serialized).unwrap();
    let trace = ProcessRuntime::execute(&plan);

    assert!(trace.succeeded());
    assert_eq!(trace.entries.len(), 1);
    assert_eq!(trace.entries[0].implementation_ref, "impl:second");
    assert_eq!(trace.entries[0].stdout.trim(), "SECOND");
}

#[test]
fn direct_concrete_ref_trace_exposes_same_concrete_implementation() {
    let graph = graph("impl:first");
    let implementations = implementations();

    let plan = Planner::lower(&graph, &implementations, &RouteRegistry::new()).unwrap();
    let trace = ProcessRuntime::execute(&plan);

    assert!(trace.succeeded());
    assert_eq!(plan.steps[0].implementation_ref, "impl:first");
    assert_eq!(trace.entries[0].implementation_ref, "impl:first");
}

#[test]
fn failed_and_not_run_entries_retain_their_concrete_implementation_refs() {
    let plan = ExecutionPlan {
        reference_graph_id: "graph:trace-status".to_owned(),
        steps: vec![
            ExecutionStep {
                id: "step:fail".to_owned(),
                implementation_ref: "impl:fail".to_owned(),
                action: ProcessAction::new(
                    "python3",
                    ["-c", "import sys; print('FAIL'); sys.exit(7)"],
                ),
                origin_block_ids: vec!["block:fail".to_owned()],
                source_anchors: Vec::new(),
                route_connector_ids: Vec::new(),
                argv_bindings: Vec::new(),
            },
            ExecutionStep {
                id: "step:not-run".to_owned(),
                implementation_ref: "impl:not-run".to_owned(),
                action: ProcessAction::new("python3", ["-c", "print('SHOULD_NOT_RUN')"]),
                origin_block_ids: vec!["block:not-run".to_owned()],
                source_anchors: Vec::new(),
                route_connector_ids: Vec::new(),
                argv_bindings: Vec::new(),
            },
        ],
    };

    let trace = ProcessRuntime::execute(&plan);

    assert!(!trace.succeeded());
    assert_eq!(trace.entries.len(), 2);
    assert_eq!(trace.entries[0].status, TraceStatus::Failed);
    assert_eq!(trace.entries[0].implementation_ref, "impl:fail");
    assert_eq!(trace.entries[1].status, TraceStatus::NotRun);
    assert_eq!(trace.entries[1].implementation_ref, "impl:not-run");
}
