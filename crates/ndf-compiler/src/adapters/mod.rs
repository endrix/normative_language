use std::path::Path;

use ndf_core::model::ParseResult;

use crate::CompilerError;
use crate::manifest::ProjectManifest;
use crate::markdown::parse_markdown;

pub mod evidence;
pub mod pto_asl;
pub mod repository;

pub trait Adapter: Sync {
    fn name(&self) -> &'static str;
    fn parse(&self, path: &Path, manifest: &ProjectManifest) -> Result<ParseResult, CompilerError>;
}

struct MarkdownAdapter;
struct PtoAslAdapter;
struct EvidenceAdapter;

impl Adapter for MarkdownAdapter {
    fn name(&self) -> &'static str {
        "markdown"
    }

    fn parse(&self, path: &Path, manifest: &ProjectManifest) -> Result<ParseResult, CompilerError> {
        parse_markdown(path, manifest)
    }
}

impl Adapter for PtoAslAdapter {
    fn name(&self) -> &'static str {
        "pto-asl"
    }

    fn parse(&self, path: &Path, manifest: &ProjectManifest) -> Result<ParseResult, CompilerError> {
        pto_asl::parse_pto_asl(path, manifest)
    }
}

impl Adapter for EvidenceAdapter {
    fn name(&self) -> &'static str {
        "evidence"
    }

    fn parse(&self, path: &Path, manifest: &ProjectManifest) -> Result<ParseResult, CompilerError> {
        evidence::parse_evidence_manifest(path, manifest)
    }
}

static MARKDOWN: MarkdownAdapter = MarkdownAdapter;
static PTO_ASL: PtoAslAdapter = PtoAslAdapter;
static EVIDENCE: EvidenceAdapter = EvidenceAdapter;

pub fn adapter_for(path: &Path) -> Option<&'static dyn Adapter> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("md") => Some(&MARKDOWN),
        Some("asl") => Some(&PTO_ASL),
        Some("json") => Some(&EVIDENCE),
        _ => None,
    }
}
