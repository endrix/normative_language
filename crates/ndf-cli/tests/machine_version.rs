use assert_cmd::Command;
use serde_json::Value;

#[test]
fn machine_version_matches_the_frozen_contract() {
    let expected: Value =
        serde_json::from_str(include_str!("../../../tests/golden/machine-version.json"))
            .expect("machine-version golden JSON must parse");

    let output = Command::new(assert_cmd::cargo::cargo_bin!("ndf"))
        .arg("--machine-version")
        .output()
        .expect("ndf must run");

    assert_eq!(
        output.status.code(),
        expected["exit_code"].as_i64().map(|value| value as i32)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).expect("ndf must emit JSON"),
        expected["payload"],
    );
}
