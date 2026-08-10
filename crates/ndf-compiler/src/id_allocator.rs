use std::collections::BTreeSet;

use crate::CompilerError;

pub fn allocate_id(prefix: &str, occupied: &BTreeSet<String>) -> Result<String, CompilerError> {
    if !valid_prefix(prefix) {
        return Err(CompilerError::Contract(format!(
            "invalid ID prefix: {prefix:?}"
        )));
    }
    let marker = format!("{prefix}-");
    let next = occupied
        .iter()
        .filter_map(|value| value.strip_prefix(&marker))
        .filter(|suffix| !suffix.is_empty() && suffix.chars().all(|item| item.is_ascii_digit()))
        .filter_map(|suffix| suffix.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    Ok(format!("{prefix}-{next:03}"))
}

fn valid_prefix(prefix: &str) -> bool {
    let characters: Vec<_> = prefix.chars().collect();
    characters.len() >= 2
        && characters[0].is_ascii_uppercase()
        && characters
            .last()
            .is_some_and(|last| last.is_ascii_uppercase() || last.is_ascii_digit())
        && characters
            .iter()
            .all(|item| item.is_ascii_uppercase() || item.is_ascii_digit() || *item == '-')
}
