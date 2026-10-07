use algoram_core::{Graph, GraphError};
use serde_json::Value;

fn code(error: &GraphError) -> &'static str {
    match error {
        GraphError::Json(_) => "json",
        GraphError::UnsupportedSchemaVersion { .. } => "unsupported-schema-version",
        GraphError::DuplicateBlockId(_) => "duplicate-block-id",
        GraphError::DuplicatePortId { .. } => "duplicate-port-id",
        GraphError::DuplicateConnectionId(_) => "duplicate-connection-id",
        GraphError::DuplicateSourceArtifactId(_) => "duplicate-source-artifact-id",
        GraphError::MissingBlock { .. } => "missing-block",
        GraphError::MissingPort { .. } => "missing-port",
        GraphError::InvalidConnectionDirection { .. } => "invalid-connection-direction",
        GraphError::ChannelMismatch { .. } => "channel-mismatch",
        GraphError::InvalidSourceAnchorRange { .. } => "invalid-source-anchor-range",
        GraphError::MissingSourceArtifact { .. } => "missing-source-artifact",
    }
}

#[test]
fn browser_validation_cases_match_graph_core_v01() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../fixtures/graph-validation/browser-core-parity.json"
    ))
    .expect("shared graph validation fixture must parse");

    assert_eq!(
        fixture["schema_version"].as_str(),
        Some("algoram.graph-validation-parity/1")
    );

    for case in fixture["cases"]
        .as_array()
        .expect("fixture cases must be an array")
    {
        let name = case["name"].as_str().expect("case name");
        let expected = case["expected"].as_str();
        let graph: Graph =
            serde_json::from_value(case["graph"].clone()).expect("case Graph must deserialize");
        let actual_error = graph.validate().err();
        let actual = actual_error.as_ref().map(code);

        assert_eq!(actual, expected, "Graph Core parity case '{name}'");
    }
}
