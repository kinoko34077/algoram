use algoram_core::{Block, Extensions, Graph};
use algoram_interop::RouteRegistry;
use algoram_runtime::{
    ImplementationRegistry, Planner, PlannerError, ProcessAction, ProcessRuntime,
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

fn one_block_graph(implementation_ref: &str) -> Graph {
    let mut graph = Graph::new("graph:implementation-choice");
    graph
        .blocks
        .push(block("block:choice-target", implementation_ref));
    graph
}

fn registered_actions() -> ImplementationRegistry {
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
}

#[test]
fn direct_concrete_ref_remains_compatible() {
    let graph = one_block_graph("impl:first");
    let implementations = registered_actions();

    let plan = Planner::lower(&graph, &implementations, &RouteRegistry::new()).unwrap();

    assert_eq!(plan.steps.len(), 1);
    assert_eq!(plan.steps[0].implementation_ref, "impl:first");
    assert_eq!(
        plan.steps[0].action,
        ProcessAction::new("python3", ["-c", "print('FIRST')"])
    );
}

#[test]
fn logical_choice_freezes_selected_concrete_ref_and_replays_without_registry() {
    let serialized = {
        let graph = one_block_graph("choice:greeting");
        let mut implementations = registered_actions();
        implementations
            .register_choice(
                "choice:greeting",
                ["impl:first", "impl:second"],
                "impl:second",
            )
            .unwrap();

        let choice = implementations.choice("choice:greeting").unwrap();
        assert_eq!(choice.candidates, vec!["impl:first", "impl:second"]);
        assert_eq!(choice.default_ref, "impl:second");

        let plan = Planner::lower(&graph, &implementations, &RouteRegistry::new()).unwrap();

        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].implementation_ref, "impl:second");
        assert_eq!(
            plan.steps[0].action,
            ProcessAction::new("python3", ["-c", "print('SECOND')"])
        );

        serde_json::to_string_pretty(&plan).unwrap()
    };

    let restored = serde_json::from_str(&serialized).unwrap();

    assert_eq!(restored.steps[0].implementation_ref, "impl:second");
    let trace = ProcessRuntime::execute(&restored);
    assert!(trace.succeeded());
    assert_eq!(trace.entries.len(), 1);
    assert_eq!(trace.entries[0].stdout.trim(), "SECOND");
}

#[test]
fn choice_key_cannot_collide_with_concrete_ref() {
    let mut implementations = registered_actions();

    let error = implementations
        .register_choice("impl:first", ["impl:first"], "impl:first")
        .unwrap_err();
    assert!(matches!(
        error,
        PlannerError::DuplicateImplementationRef(reference)
            if reference == "impl:first"
    ));
}

#[test]
fn concrete_ref_cannot_collide_with_existing_choice_key() {
    let mut implementations = registered_actions();
    implementations
        .register_choice(
            "choice:greeting",
            ["impl:first", "impl:second"],
            "impl:first",
        )
        .unwrap();

    let error = implementations
        .register(
            "choice:greeting",
            ProcessAction::new("python3", ["-c", "pass"]),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        PlannerError::DuplicateImplementationRef(reference)
            if reference == "choice:greeting"
    ));
}

#[test]
fn empty_choice_is_rejected() {
    let mut implementations = registered_actions();

    let error = implementations
        .register_choice(
            "choice:empty",
            std::iter::empty::<&str>(),
            "impl:first",
        )
        .unwrap_err();
    assert!(matches!(
        error,
        PlannerError::EmptyImplementationChoice { logical_ref }
            if logical_ref == "choice:empty"
    ));
}

#[test]
fn duplicate_candidate_is_rejected() {
    let mut implementations = registered_actions();

    let error = implementations
        .register_choice(
            "choice:duplicate",
            ["impl:first", "impl:first"],
            "impl:first",
        )
        .unwrap_err();
    assert!(matches!(
        error,
        PlannerError::DuplicateImplementationCandidate {
            logical_ref,
            candidate_ref
        } if logical_ref == "choice:duplicate" && candidate_ref == "impl:first"
    ));
}

#[test]
fn unregistered_candidate_is_rejected() {
    let mut implementations = registered_actions();

    let error = implementations
        .register_choice(
            "choice:missing",
            ["impl:first", "impl:missing"],
            "impl:first",
        )
        .unwrap_err();
    assert!(matches!(
        error,
        PlannerError::MissingImplementationCandidate {
            logical_ref,
            candidate_ref
        } if logical_ref == "choice:missing" && candidate_ref == "impl:missing"
    ));
}

#[test]
fn default_must_belong_to_candidates() {
    let mut implementations = registered_actions();

    let error = implementations
        .register_choice("choice:default", ["impl:first"], "impl:second")
        .unwrap_err();
    assert!(matches!(
        error,
        PlannerError::DefaultImplementationNotCandidate {
            logical_ref,
            default_ref
        } if logical_ref == "choice:default" && default_ref == "impl:second"
    ));
}
