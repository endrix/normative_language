import tempfile
import unittest
from pathlib import Path

from ndf.manifest import ProjectManifest, load_manifest
from ndf.model import DiagnosticSeverity, Modality, NodeKind
from ndf.parsers.markdown import parse_markdown


class MarkdownParserTest(unittest.TestCase):
    def setUp(self) -> None:
        self.project_root = (
            Path(__file__).parent / "fixtures" / "markdown" / "project"
        )
        self.manifest = load_manifest(self.project_root / "ndf.yaml")

    def test_complete_document_produces_nodes_edges_and_source_spans(self) -> None:
        result = parse_markdown(
            self.project_root / "docs" / "pipeline.md",
            self.manifest,
        )

        self.assertEqual(len(result.nodes), 3)
        self.assertEqual(len(result.edges), 1)
        self.assertEqual(result.diagnostics, ())
        self.assertEqual(
            [node.id.local_id for node in result.nodes],
            ["DOC-FIXTURE-PIPELINE", "PIPE-ISSUE-001", "PIPE-ISSUE-010"],
        )
        self.assertNotIn("FAKE-001", [node.id.local_id for node in result.nodes])
        self.assertEqual(result.nodes[1].source.line, 14)
        self.assertEqual(result.nodes[2].source.line, 19)
        self.assertEqual(result.nodes[1].kind, NodeKind.REQUIREMENT)
        self.assertEqual(result.nodes[1].modality, Modality.MUST)
        self.assertEqual(str(result.edges[0].source), "ndf://fixture/PIPE-ISSUE-010")
        self.assertEqual(str(result.edges[0].target), "ndf://fixture/PIPE-ISSUE-001")
        self.assertEqual(result.edges[0].type, "refines")

    def test_unclosed_metadata_comment_emits_delimiter_diagnostic(self) -> None:
        result = self._parse_text(
            """---
doc_id: DOC-FIXTURE-BROKEN
status: active
authority: normative
owner: core
---
# Broken
## Contract {#PIPE-BROKEN-001}
<!-- ndf: kind=requirement modality=must
The core MUST stop.
"""
        )

        self.assertEqual([item.code for item in result.diagnostics], ["NDF-MD-001"])
        self.assertEqual(result.diagnostics[0].severity, DiagnosticSeverity.ERROR)

    def test_duplicate_metadata_key_emits_stable_diagnostic(self) -> None:
        result = self._parse_text(
            self._single_clause("kind=requirement kind=decision modality=must")
        )

        self.assertEqual([item.code for item in result.diagnostics], ["NDF-MD-002"])

    def test_unsupported_metadata_field_emits_stable_diagnostic(self) -> None:
        result = self._parse_text(
            self._single_clause("kind=requirement modality=must priority=high")
        )

        self.assertEqual([item.code for item in result.diagnostics], ["NDF-MD-003"])

    def test_legacy_davincioo_fields_are_mapped_with_migration_diagnostic(self) -> None:
        result = self._parse_text(
            self._single_clause("kind=req level=must layer=L1 status=stable")
        )

        clause = result.nodes[1]
        self.assertEqual(clause.kind, NodeKind.REQUIREMENT)
        self.assertEqual(clause.modality, Modality.MUST)
        self.assertEqual(clause.refinement, "L1")
        self.assertEqual([item.code for item in result.diagnostics], ["NDF-MIG-001"])
        self.assertEqual(
            result.diagnostics[0].severity,
            DiagnosticSeverity.INFORMATION,
        )

    def _parse_text(self, text: str):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "document.md"
            path.write_text(text)
            manifest = ProjectManifest(
                format_version="0.2",
                project="fixture",
                roots=("*.md",),
                id_prefixes=("DOC", "PIPE"),
                domains=("core",),
                policies=(),
                root=root,
            )
            return parse_markdown(path, manifest)

    @staticmethod
    def _single_clause(metadata: str) -> str:
        return f"""---
doc_id: DOC-FIXTURE-ONE
status: active
authority: normative
owner: core
---
# One
## Contract {{#PIPE-ONE-001}}
<!-- ndf: {metadata} -->
The core MUST stop.
"""


if __name__ == "__main__":
    unittest.main()
