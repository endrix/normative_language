use std::path::{Path, PathBuf};

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn release_tree_has_no_python_ndf_runtime() {
    let root = repository();
    for forbidden in ["pyproject.toml", "requirements-lock.txt", "src/ndf"] {
        assert!(
            !root.join(forbidden).exists(),
            "{forbidden} must be removed"
        );
    }
    assert!(root.join("Cargo.lock").is_file());
    assert!(root.join("scripts/ndf").is_file());
}
