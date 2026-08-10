use ndf_compiler::baseline::{
    BaselineEntry, DiagnosticBaseline, compare_baseline, diagnostic_fingerprint,
};
use ndf_core::identity::NodeId;
use ndf_core::model::{Diagnostic, DiagnosticSeverity, SourceSpan};

fn diagnostic(severity: DiagnosticSeverity, line: u32) -> Diagnostic {
    Diagnostic {
        code: "NDF-MOD-001".to_owned(),
        severity,
        message: "requirement  ndf://fixture/PIPE-001\nis invalid".to_owned(),
        primary: SourceSpan::new("docs\\pipeline.md", line).unwrap(),
        related: Vec::new(),
        repair: None,
        subject: Some(NodeId::new("fixture", "PIPE-001").unwrap()),
    }
}

#[test]
fn fingerprint_ignores_line_movement_path_separators_and_whitespace_runs() {
    let before = diagnostic(DiagnosticSeverity::Warning, 10);
    let mut after = diagnostic(DiagnosticSeverity::Warning, 80);
    after.primary.path = "docs/pipeline.md".to_owned();
    after.message = "requirement ndf://fixture/PIPE-001 is invalid".to_owned();

    assert_eq!(
        diagnostic_fingerprint(&before),
        diagnostic_fingerprint(&after)
    );
}

#[test]
fn baseline_classifies_existing_new_and_worsened_diagnostics() {
    let warning = diagnostic(DiagnosticSeverity::Warning, 10);
    let baseline = DiagnosticBaseline {
        entries: vec![BaselineEntry::from_diagnostic(
            &warning,
            "core",
            "2099-12-31",
        )],
    };

    let existing = compare_baseline(std::slice::from_ref(&warning), &baseline);
    assert_eq!(existing.existing.as_slice(), std::slice::from_ref(&warning));

    let mut new_code = warning.clone();
    new_code.code = "NDF-OWN-001".to_owned();
    let new_result = compare_baseline(&[new_code.clone()], &baseline);
    assert_eq!(new_result.new, [new_code]);

    let error = diagnostic(DiagnosticSeverity::Error, 70);
    let worsened = compare_baseline(std::slice::from_ref(&error), &baseline);
    assert_eq!(worsened.worsened, [error]);
}

#[test]
fn baseline_requires_owner_and_valid_expiry() {
    let current = diagnostic(DiagnosticSeverity::Warning, 10);
    let baseline = DiagnosticBaseline {
        entries: vec![BaselineEntry {
            fingerprint: diagnostic_fingerprint(&current),
            code: current.code.clone(),
            severity: current.severity,
            subject: current.subject.as_ref().unwrap().to_string(),
            path: current.primary.path.clone(),
            owner: None,
            expires: None,
        }],
    };

    let result = compare_baseline(&[current], &baseline);

    assert_eq!(result.new[0].code, "NDF-BASE-003");
}
