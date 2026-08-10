use std::fs;
use std::path::{Path, PathBuf};

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn english_and_chinese_publish_the_same_machine_contract() {
    let root = repository();
    let english = contract(&root.join("normative_language.md"));
    let chinese = contract(&root.join("normative_language_cn.md"));
    assert_eq!(english, chinese);
    assert_eq!(
        english,
        r#"format_version: "0.2"
ir_version: "0.1"
machine_output_schema: "0.1"
derived_index: sqlite-v1
identity_scheme: ndf
canonical_fields:
  - kind
  - modality
  - refinement
  - domain
  - status
required_files:
  - ndf.yaml
  - ndf.lock
agent_gate:
  - build
  - check
  - diff
  - coverage
maturity:
  format: experimental
  ir: experimental
  cli: experimental
  plugin_api: experimental"#
    );
}

fn contract(path: &Path) -> String {
    let text = fs::read_to_string(path).unwrap();
    let start_marker = "<!-- ndf:format-contract:start -->";
    let end_marker = "<!-- ndf:format-contract:end -->";
    let start = text.find(start_marker).unwrap() + start_marker.len();
    let end = text[start..].find(end_marker).unwrap() + start;
    let block = text[start..end].trim();
    block
        .strip_prefix("```yaml\n")
        .and_then(|value| value.strip_suffix("\n```"))
        .expect("format contract must be one YAML code block")
        .to_owned()
}
