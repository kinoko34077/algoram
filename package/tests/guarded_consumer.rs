use algoram_composite::CompositeDefinition;
use algoram_core::{Block, Extensions, Graph, SourceAnchor, SourceArtifact};
use algoram_interop::RouteRegistry;
use algoram_package::{BlockPackage, PackageProvenance};
use algoram_runtime::{
    ExecutionAccessClass, ExecutionPlan, ExecutionPolicy, ExecutionSecurityError,
    GuardedProcessRuntime, ImplementationRegistry, ProcessAction, TraceStatus,
};
use std::fs;
use std::path::Path;

fn producer_json() -> String {
    let mut internal_graph = Graph::new("graph:guarded-portable-package");
    internal_graph.source_artifacts.push(SourceArtifact {
        id: "artifact:guarded-portable-source".to_owned(),
        origin: "src/guarded_portable.py".to_owned(),
        language: "python".to_owned(),
        revision: Some("source-rev-security-1".to_owned()),
        content_hash: Some("sha256:guarded-portable".to_owned()),
        repository_origin: Some("producer/security-repository".to_owned()),
        license: Some("MIT".to_owned()),
    });

    internal_graph.blocks.push(Block {
        id: "block:guarded-packaged-executable".to_owned(),
        label: "Guarded packaged executable".to_owned(),
        ports: Vec::new(),
        internal_graph_ref: None,
        implementation_ref: Some("impl:guarded-package".to_owned()),
        definition_ref: None,
        source_anchor: Some(SourceAnchor {
            artifact_id: "artifact:guarded-portable-source".to_owned(),
            start_byte: 0,
            end_byte: 32,
            start_line: Some(0),
            start_column: Some(0),
            end_line: Some(1),
            end_column: Some(0),
            semantic_key: Some("python:guarded_portable.main".to_owned()),
        }),
        extensions: Extensions::new(),
        diagnostics: Vec::new(),
    });

    let definition = CompositeDefinition::new(
        "definition:guarded-portable-package",
        "Guarded portable package",
        internal_graph,
        Vec::new(),
        Vec::new(),
    );

    BlockPackage::new("example.guarded-portable", "1", definition)
        .with_provenance(PackageProvenance {
            origin: Some("producer://guarded-portable-package".to_owned()),
            revision: Some("package-security-rev-1".to_owned()),
            license: Some("Apache-2.0".to_owned()),
        })
        .to_json_pretty()
        .unwrap()
}

fn marker_action(marker: &Path, value: &str) -> ProcessAction {
    ProcessAction::new(
        "python3",
        [
            "-c".to_owned(),
            "from pathlib import Path; import sys; Path(sys.argv[1]).write_text(sys.argv[2]); print(sys.argv[2])"
                .to_owned(),
            marker.display().to_string(),
            value.to_owned(),
        ],
    )
}

#[test]
fn json_only_imported_package_is_default_denied_then_explicitly_guarded() {
    let serialized = producer_json();
    let package = BlockPackage::from_json(&serialized).unwrap();

    let artifact = &package.definition.internal_graph.source_artifacts[0];
    assert_eq!(artifact.origin, "src/guarded_portable.py");
    assert_eq!(artifact.revision.as_deref(), Some("source-rev-security-1"));
    assert_eq!(
        artifact.repository_origin.as_deref(),
        Some("producer/security-repository")
    );
    assert_eq!(artifact.license.as_deref(), Some("MIT"));

    let instance = package
        .definition
        .instantiate("block:guarded-consumer-instance", "Guarded imported instance")
        .unwrap();

    let marker = std::env::temp_dir().join(format!(
        "algoram-guarded-package-allowed-{}",
        std::process::id()
    ));
    let _ = fs::remove_file(&marker);

    let trusted_action = marker_action(&marker, "GUARDED_PACKAGE_OK");
    let mut trusted = ImplementationRegistry::new();
    trusted
        .register("impl:guarded-package", trusted_action.clone())
        .unwrap();

    let plan = package
        .definition
        .lower_instance(&instance, &trusted, &RouteRegistry::new())
        .unwrap();

    let report = GuardedProcessRuntime::inspect(&plan, &trusted).unwrap();
    assert_eq!(report.requirements.len(), 1);
    let requirement = &report.requirements[0];
    assert_eq!(requirement.implementation_ref, "impl:guarded-package");
    assert_eq!(
        requirement.access_class,
        ExecutionAccessClass::AmbientHostProcess
    );
    assert_eq!(requirement.action, trusted_action);
    assert_eq!(
        requirement.origin_block_ids,
        vec![
            "block:guarded-consumer-instance",
            "block:guarded-packaged-executable"
        ]
    );

    let denied =
        GuardedProcessRuntime::execute(&plan, &trusted, &ExecutionPolicy::new()).unwrap_err();
    assert!(matches!(
        denied,
        ExecutionSecurityError::DeniedImplementation {
            implementation_ref,
            ..
        } if implementation_ref == "impl:guarded-package"
    ));
    assert!(!marker.exists());

    let mut policy = ExecutionPolicy::new();
    policy.allow("impl:guarded-package");

    let trace = GuardedProcessRuntime::execute(&plan, &trusted, &policy).unwrap();
    assert!(trace.succeeded());
    assert_eq!(trace.entries.len(), 1);
    assert_eq!(trace.entries[0].status, TraceStatus::Succeeded);
    assert_eq!(
        trace.entries[0].implementation_ref,
        "impl:guarded-package"
    );
    assert_eq!(
        trace.entries[0].origin_block_ids,
        vec![
            "block:guarded-consumer-instance",
            "block:guarded-packaged-executable"
        ]
    );
    assert_eq!(
        trace.entries[0].source_anchors[0].artifact_id,
        "artifact:guarded-portable-source"
    );
    assert_eq!(
        trace.entries[0]
            .source_anchors[0]
            .semantic_key
            .as_deref(),
        Some("python:guarded_portable.main")
    );
    assert_eq!(trace.entries[0].stdout.trim(), "GUARDED_PACKAGE_OK");
    assert_eq!(fs::read_to_string(&marker).unwrap(), "GUARDED_PACKAGE_OK");

    let _ = fs::remove_file(&marker);
}

#[test]
fn tampered_serialized_plan_cannot_reuse_allowed_implementation_authority() {
    let package = BlockPackage::from_json(&producer_json()).unwrap();
    let instance = package
        .definition
        .instantiate("block:tamper-consumer-instance", "Tamper imported instance")
        .unwrap();

    let trusted_marker = std::env::temp_dir().join(format!(
        "algoram-guarded-package-trusted-{}",
        std::process::id()
    ));
    let tampered_marker = std::env::temp_dir().join(format!(
        "algoram-guarded-package-tampered-{}",
        std::process::id()
    ));
    let _ = fs::remove_file(&trusted_marker);
    let _ = fs::remove_file(&tampered_marker);

    let trusted_action = marker_action(&trusted_marker, "TRUSTED");
    let mut trusted = ImplementationRegistry::new();
    trusted
        .register("impl:guarded-package", trusted_action)
        .unwrap();

    let plan = package
        .definition
        .lower_instance(&instance, &trusted, &RouteRegistry::new())
        .unwrap();

    let mut serialized_plan = serde_json::to_value(&plan).unwrap();
    serialized_plan["steps"][0]["action"] =
        serde_json::to_value(marker_action(&tampered_marker, "TAMPERED")).unwrap();
    let tampered_plan: ExecutionPlan = serde_json::from_value(serialized_plan).unwrap();

    assert_eq!(
        tampered_plan.steps[0].implementation_ref,
        "impl:guarded-package"
    );

    let mut policy = ExecutionPolicy::new();
    policy.allow("impl:guarded-package");

    let error = GuardedProcessRuntime::execute(&tampered_plan, &trusted, &policy).unwrap_err();
    assert!(matches!(
        error,
        ExecutionSecurityError::TrustedActionMismatch {
            implementation_ref,
            ..
        } if implementation_ref == "impl:guarded-package"
    ));
    assert!(!trusted_marker.exists());
    assert!(!tampered_marker.exists());
}
