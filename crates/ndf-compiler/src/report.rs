use serde::Serialize;

use crate::CompilerError;
use crate::index::NdfIndex;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CoveragePolicy {
    pub refinement: String,
}

impl Default for CoveragePolicy {
    fn default() -> Self {
        Self {
            refinement: "L1".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct CoverageReport {
    pub unverified: Vec<String>,
    pub open_items: Vec<String>,
    pub tbd: Vec<String>,
}

pub fn coverage(
    index: &NdfIndex,
    policy: &CoveragePolicy,
) -> Result<CoverageReport, CompilerError> {
    let uris = index.coverage_uris(&policy.refinement)?;
    Ok(CoverageReport {
        unverified: uris.unverified,
        open_items: uris.open_items,
        tbd: uris.tbd,
    })
}
