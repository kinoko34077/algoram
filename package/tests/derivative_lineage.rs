use algoram_composite::CompositeDefinition;
use algoram_core::{Block, Extensions, Graph, SourceAnchor, SourceArtifact};
use algoram_package::{BlockPackage, PackageProvenance, PackageRef};

fn upstream_json() -> String {
    let mut graph = Graph::new("graph:upstream-open");
    graph.source_artifacts.push(SourceArtifact {
        id: "artifact:upstream".to_owned(),
        origin: "src/upstream.py".to_owned(),
        language: "python".to_owned(),
        revision: Some("source-rev-a".to_owned()),
        content_hash: Some("sha256:source-a".to_owned()),
        repository_origin: Some("upstream/source-repo".to_owned()),
        license: Some("MIT".to_owned()),
    });

    graph.blocks.push(Block {
        id: "block:open-function".to_owned(),
        label: "Open function".to_owned(),
        ports: Vec::new(),
        internal_graph_ref: None,
        implementation_ref: Some("impl:open-function".to_owned()),
        definition_ref: None,
        source_anchor: Some(SourceAnchor {
            artifact_id: "artifact:upstream".to_owned(),
            start_byte: 0,
            end_byte: 20,
            start_line: Some(0),
            start_column: Some(0),
            end_line: Some(1),
            end_column: Some(0),
            semantic_key: Some("python:upstream.fn".to_owned()),
        }),
        extensions: Extensions::new(),
        diagnostics: Vec::new(),
    });

    let definition = CompositeDefinition::new(
        "definition:upstream-open",
        "Upstream open definition",
        graph,
        Vec::new(),
        Vec::new(),
    );

    BlockPackage::new("example.upstream", "1", definition)
        .with_provenance(PackageProvenance {
            origin: Some("https://packages.example/upstream".to_owned()),
            revision: Some("pkg-upstream-rev".to_owned()),
            license: Some("Apache-2.0".to_owned()),
        })
        .to_json_pretty()
        .unwrap()
}

#[test]
fn imported_open_package_can_fork_with_lineage_without_mutating_upstream() {
    let upstream = BlockPackage::from_json(&upstream_json()).unwrap();
    let upstream_snapshot = upstream.clone();
    let upstream_json_before = upstream.to_json_pretty().unwrap();

    let upstream_ref = PackageRef {
        package_id: upstream.package_id.clone(),
        package_version: upstream.package_version.clone(),
    };

    let mut derivative = upstream.clone();
    derivative.package_id = "example.derivative".to_owned();
    derivative.package_version = "2".to_owned();
    derivative.derived_from = Some(upstream_ref.clone());
    derivative.provenance = PackageProvenance {
        origin: Some("https://packages.example/derivative".to_owned()),
        revision: Some("pkg-derivative-rev".to_owned()),
        license: Some("BSD-3-Clause".to_owned()),
    };
    derivative.definition.label = "Derived open definition".to_owned();
    derivative.definition.internal_graph.blocks[0].label =
        "Modified derivative function".to_owned();
    derivative
        .definition
        .internal_graph
        .extensions
        .insert("derivative.note".to_owned(), serde_json::json!("modified"));

    derivative.validate().unwrap();

    assert_eq!(upstream, upstream_snapshot);
    assert_eq!(upstream.to_json_pretty().unwrap(), upstream_json_before);

    assert_ne!(derivative.package_id, upstream.package_id);
    assert_ne!(derivative.package_version, upstream.package_version);
    assert_eq!(derivative.derived_from, Some(upstream_ref));

    assert_eq!(
        upstream.definition.internal_graph.blocks[0].label,
        "Open function"
    );
    assert_eq!(
        derivative.definition.internal_graph.blocks[0].label,
        "Modified derivative function"
    );
    assert!(!upstream
        .definition
        .internal_graph
        .extensions
        .contains_key("derivative.note"));

    assert_eq!(
        upstream.provenance.origin.as_deref(),
        Some("https://packages.example/upstream")
    );
    assert_eq!(upstream.provenance.license.as_deref(), Some("Apache-2.0"));
    assert_eq!(
        derivative.provenance.origin.as_deref(),
        Some("https://packages.example/derivative")
    );
    assert_eq!(
        derivative.provenance.license.as_deref(),
        Some("BSD-3-Clause")
    );

    let source = &derivative.definition.internal_graph.source_artifacts[0];
    assert_eq!(source.origin, "src/upstream.py");
    assert_eq!(source.revision.as_deref(), Some("source-rev-a"));
    assert_eq!(
        source.repository_origin.as_deref(),
        Some("upstream/source-repo")
    );
    assert_eq!(source.license.as_deref(), Some("MIT"));

    let anchor = derivative.definition.internal_graph.blocks[0]
        .source_anchor
        .as_ref()
        .unwrap();
    assert_eq!(anchor.semantic_key.as_deref(), Some("python:upstream.fn"));

    let serialized_derivative = derivative.to_json_pretty().unwrap();
    let restored_derivative = BlockPackage::from_json(&serialized_derivative).unwrap();
    assert_eq!(restored_derivative, derivative);

    assert_eq!(
        restored_derivative.derived_from,
        Some(PackageRef {
            package_id: "example.upstream".to_owned(),
            package_version: "1".to_owned(),
        })
    );
    assert_eq!(
        restored_derivative.definition.internal_graph.source_artifacts[0]
            .license
            .as_deref(),
        Some("MIT")
    );
    assert_eq!(
        restored_derivative.provenance.license.as_deref(),
        Some("BSD-3-Clause")
    );
}
