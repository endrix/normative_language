import unittest

from ndf.baseline import (
    BaselineEntry,
    DiagnosticBaseline,
    compare_baseline,
    diagnostic_fingerprint,
)
from ndf.identity import NodeId
from ndf.model import Diagnostic, DiagnosticSeverity, SourceSpan


class DiagnosticBaselineTest(unittest.TestCase):
    def test_identical_diagnostic_is_classified_as_existing(self) -> None:
        diagnostic = self._diagnostic("NDF-MOD-001", DiagnosticSeverity.WARNING)
        baseline = DiagnosticBaseline(
            entries=(
                BaselineEntry.from_diagnostic(
                    diagnostic,
                    owner="core",
                    expires="2099-12-31",
                ),
            )
        )

        result = compare_baseline((diagnostic,), baseline)

        self.assertEqual(result.existing, (diagnostic,))
        self.assertEqual(result.new, ())
        self.assertEqual(result.worsened, ())

    def test_new_diagnostic_code_is_rejected(self) -> None:
        existing = self._diagnostic("NDF-MOD-001", DiagnosticSeverity.WARNING)
        new = self._diagnostic("NDF-OWN-001", DiagnosticSeverity.ERROR)
        baseline = DiagnosticBaseline(
            entries=(
                BaselineEntry.from_diagnostic(
                    existing,
                    owner="core",
                    expires="2099-12-31",
                ),
            )
        )

        result = compare_baseline((new,), baseline)

        self.assertEqual(result.new, (new,))

    def test_severity_increase_is_classified_as_worsened(self) -> None:
        warning = self._diagnostic("NDF-MOD-001", DiagnosticSeverity.WARNING)
        error = self._diagnostic("NDF-MOD-001", DiagnosticSeverity.ERROR)
        baseline = DiagnosticBaseline(
            entries=(
                BaselineEntry.from_diagnostic(
                    warning,
                    owner="core",
                    expires="2099-12-31",
                ),
            )
        )

        result = compare_baseline((error,), baseline)

        self.assertEqual(result.worsened, (error,))

    def test_missing_owner_or_expiry_emits_baseline_integrity_error(self) -> None:
        diagnostic = self._diagnostic("NDF-MOD-001", DiagnosticSeverity.WARNING)
        baseline = DiagnosticBaseline(
            entries=(
                BaselineEntry(
                    fingerprint=diagnostic_fingerprint(diagnostic),
                    code=diagnostic.code,
                    severity=diagnostic.severity,
                    subject=str(diagnostic.subject),
                    path=diagnostic.primary.path,
                    owner=None,
                    expires=None,
                ),
            )
        )

        result = compare_baseline((diagnostic,), baseline)

        self.assertEqual([item.code for item in result.new], ["NDF-BASE-003"])

    def test_fingerprint_ignores_line_movement(self) -> None:
        before = self._diagnostic("NDF-MOD-001", DiagnosticSeverity.WARNING, line=10)
        after = self._diagnostic("NDF-MOD-001", DiagnosticSeverity.WARNING, line=80)

        self.assertEqual(
            diagnostic_fingerprint(before),
            diagnostic_fingerprint(after),
        )

    @staticmethod
    def _diagnostic(
        code: str,
        severity: DiagnosticSeverity,
        *,
        line: int = 10,
    ) -> Diagnostic:
        return Diagnostic(
            code=code,
            severity=severity,
            message="requirement ndf://fixture/PIPE-001 is invalid",
            primary=SourceSpan("docs/pipeline.md", line),
            subject=NodeId("fixture", "PIPE-001"),
        )


if __name__ == "__main__":
    unittest.main()
