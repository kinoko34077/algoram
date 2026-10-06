use algoram_core::Graph;
use algoram_interop::{
    Connector, ContractId, RouteRegistry, RouteRequest, TransferMode,
};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("composite crate has repository parent")
        .to_path_buf()
}

fn accepted_route_graph() -> Graph {
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
                "c-abi-call-algoram-checked-double",
                ContractId::from("c:abi:int32"),
                ContractId::from("c:function:algoram_checked_double:int32"),
                "fixture:c:function:algoram_checked_double",
            )
            .with_transfer(TransferMode::Copy),
        )
        .unwrap();

    registry
        .resolve(&RouteRequest::automatic(
            "python:ctypes:c_int",
            "c:function:algoram_checked_double:int32",
        ))
        .unwrap()
        .route()
        .expect("accepted prototype route resolves")
        .to_inspection_graph()
        .unwrap()
}

#[test]
fn checked_in_ui_route_graph_matches_accepted_interop_projection() {
    let path = repo_root().join("ui/src/generated/phase1-route.json");
    let text = fs::read_to_string(path).expect("checked-in UI route fixture exists");
    let checked_in: Graph =
        serde_json::from_str(&text).expect("checked-in UI route fixture is valid Graph JSON");

    let expected = accepted_route_graph();

    assert_eq!(checked_in, expected);
    checked_in.validate().unwrap();
}

#[test]
fn checked_in_ui_route_fixture_contains_no_manual_equivalence_claim() {
    let path = repo_root().join("ui/src/generated/phase1-route.json");
    let text = fs::read_to_string(path).expect("checked-in UI route fixture exists");
    let value: Value = serde_json::from_str(&text).unwrap();

    let serialized = serde_json::to_string(&value).unwrap();
    assert!(!serialized.contains("equivalent"));
    assert!(!serialized.contains("substitutable"));
}
