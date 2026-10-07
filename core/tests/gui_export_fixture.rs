use algoram_core::Graph;
use serde_json::json;

const PORTABLE_GUI_EXPORT: &str =
    include_str!("../../fixtures/gui-export/portable.algoram.json");

#[test]
fn gui_export_fixture_is_accepted_by_graph_from_json() {
    let graph = Graph::from_json(PORTABLE_GUI_EXPORT)
        .expect("browser-exported Graph fixture must be accepted by Graph::from_json");

    assert_eq!(graph.schema_version, "algoram.graph/0.1");
    assert_eq!(graph.id, "graph:gui-export/portable");
    assert_eq!(graph.blocks.len(), 2);
    assert_eq!(graph.connections.len(), 1);
    assert_eq!(
        graph.extensions.get("future.vendor"),
        Some(&json!({"opaque": [1, 2, 3], "flag": true}))
    );
    assert_eq!(
        graph.presentation.get("canonical.note"),
        Some(&json!("preserve-me"))
    );

    let pretty = graph
        .to_json_pretty()
        .expect("accepted GUI export fixture must remain serializable");
    let restored =
        Graph::from_json(&pretty).expect("Rust round-trip of GUI export fixture must remain valid");
    assert_eq!(restored, graph);
}
