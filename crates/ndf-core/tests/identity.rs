use ndf_core::identity::NodeId;
use serde_json::Value;

#[test]
fn identity_behavior_matches_the_frozen_contract() {
    let expected: Value = serde_json::from_str(include_str!("../../../tests/golden/identity.json"))
        .expect("identity golden JSON must parse");

    let local = NodeId::parse("VEC-ISSUE-001", Some("davincioo"))
        .expect("local ID must resolve inside a project");
    let canonical = NodeId::parse("ndf://pto-spec/PTO-INST-TLOAD", None)
        .expect("canonical URI must parse without a current project");

    assert_eq!(local.to_string(), expected["local"].as_str().unwrap());
    assert_eq!(
        canonical.project_id(),
        expected["canonical"]["project_id"].as_str().unwrap()
    );
    assert_eq!(
        canonical.local_id(),
        expected["canonical"]["local_id"].as_str().unwrap()
    );
    assert_eq!(
        canonical.to_string(),
        expected["canonical"]["uri"].as_str().unwrap()
    );
    assert_eq!(
        NodeId::parse("VEC-ISSUE-001", None)
            .expect_err("local ID without project must fail")
            .to_string(),
        expected["errors"]["missing_project"].as_str().unwrap(),
    );
    assert_eq!(
        NodeId::new("DavinciOO", "PIPE-001")
            .expect_err("uppercase project must fail")
            .to_string(),
        expected["errors"]["invalid_project"].as_str().unwrap(),
    );
    assert_eq!(
        NodeId::parse("ndf://pto-spec/path/with/slash", None)
            .expect_err("URI with nested path must fail")
            .to_string(),
        expected["errors"]["invalid_uri"].as_str().unwrap(),
    );
    assert_eq!(
        NodeId::parse("ndf://DavinciOO/PIPE-001", None)
            .expect_err("canonical URI must retain project validation")
            .to_string(),
        "invalid project ID: 'DavinciOO'",
    );
}

#[test]
fn node_id_serializes_as_its_canonical_uri() {
    let id = NodeId::new("davincioo", "PIPE-001").unwrap();

    assert_eq!(
        serde_json::to_value(id).unwrap(),
        "ndf://davincioo/PIPE-001"
    );
}
