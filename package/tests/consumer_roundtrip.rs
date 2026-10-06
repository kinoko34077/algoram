use algoram_composite::CompositeDefinition;
use algoram_core::{Block, Extensions, Graph, SourceAnchor, SourceArtifact};
use algoram_interop::RouteRegistry;
use algoram_package::{BlockPackage, PackageProvenance};
use algoram_runtime::{ImplementationRegistry, ProcessAction, TraceStatus};

fn producer_json() -> String {
    let mut internal_graph = Graph::new("graph:portable-package");
    internal_graph.source_artifacts.push(SourceArtifact {
        id: "artifact:portable-source".to_owned(),
        origin: "src/portable.py".to_owned(),
        language: "python".to_owned(),
        revision: Some("source-rev-1".to_owned()),
        content_hash: Some("sha256:portable".to_owned()),
        repository_origin: Some("producer/repository".to_owned()),
        license: Some("MIT".to_owned()),
    });

    let executable = Block {
        id: "block:packaged-executable".to_owned(),
        label: "Packaged executable".to_owned(),
        ports: Vec::new(),
        internal_graph_ref: None,
        implementation_ref: Some("impl:packaged-hello".to_owned()),
        definition_ref: None,
        source_anchor: Some(SourceAnchor {
            artifact_id: "artifact:portable-source".to_owned(),
            start_byte: 0,
            end_byte: 24,
            start_line: Some(0),
            start_column: Some(0),
            end_line: Some(1),
            end_column: Some(0),
            semantic_key: Some("python:portable.main".to_owned()),
        }),
        extensions: Extensions::new(),
        diagnostics: Vec::new(),
    };
    internal_graph.blocks.push(executable);

    let definition = CompositeDefinition::new(
        "definition:portable-package",
        "Portable package",
        internal_graph,
        Vec::new(),
        Vec::new(),
    );

    BlockPackage::new("example.portable", "1", definition)
        .with_provenance(PackageProvenance {
            origin: Some("producer://portable-package".to_owned()),
            revision: Some("package-rev-1".to_owned()),
            license: Some("Apache-2.0".to_owned()),
        })
        .to_json_pretty()
        .unwrap()
}

#[test]
fn json_only_consumer_can_inspect_instantiate_compose_and_execute_package() {
    let serialized = producer_json();

    assert!(!serialized.contains("ImplementationRegistry"));
    assert!(!serialized.contains("RouteRegistry"));
    assert!(!serialized.contains("ImplementationProfileStore"));
    assert!(!serialized.contains("ExecutionTrace"));

    let package = BlockPackage::from_json(&serialized).unwrap();

    let artifact = &package.definition.internal_graph.source_artifacts[0];
    assert_eq!(artifact.origin, "src/portable.py");
    assert_eq!(artifact.revision.as_deref(), Some("source-rev-1"));
    assert_eq!(
        artifact.repository_origin.as_deref(),
        Some("producer/repository")
    );
    assert_eq!(artifact.license.as_deref(), Some("MIT"));

    let executable = package
        .definition
        .internal_graph
        .blocks
        .iter()
        .find(|block| block.id == "block:packaged-executable")
        .unwrap();
    assert_eq!(
        executable
            .source_anchor
            .as_ref()
            .and_then(|anchor| anchor.semantic_key.as_deref()),
        Some("python:portable.main")
    );

    let instance = package
        .definition
        .instantiate("block:consumer-instance", "Imported package instance")
        .unwrap();

    let mut parent = Graph::new("graph:consumer-parent");
    parent.blocks.push(instance.clone());
    parent.validate().unwrap();

    assert_eq!(
        parent.blocks[0].definition_ref.as_deref(),
        Some("definition:portable-package")
    );
    assert_eq!(
        parent.blocks[0].internal_graph_ref.as_deref(),
        Some("graph:portable-package")
    );

    let mut implementations = ImplementationRegistry::new();
    implementations
        .register(
            "impl:packaged-hello",
            ProcessAction::new("python3", ["-c", "print('PACKAGE_CONSUMER_OK')"]),
        )
        .unwrap();
    let routes = RouteRegistry::new();

    let trace = package
        .definition
        .execute_instance(&instance, &implementations, &routes)
        .unwrap();

    assert!(trace.succeeded());
    assert_eq!(trace.entries.len(), 1);
    assert_eq!(trace.entries[0].status, TraceStatus::Succeeded);
    assert_eq!(trace.entries[0].stdout.trim(), "PACKAGE_CONSUMER_OK");
    assert_eq!(trace.entries[0].implementation_ref, "impl:packaged-hello");
    assert_eq!(
        trace.entries[0].origin_block_ids,
        vec!["block:consumer-instance", "block:packaged-executable"]
    );
    assert_eq!(
        trace.entries[0].source_anchors[0].artifact_id,
        "artifact:portable-source"
    );
}
