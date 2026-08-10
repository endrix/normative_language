import json
import unittest
from pathlib import Path

from ndf.adapters.evidence import parse_evidence_manifest
from ndf.manifest import ProjectManifest
from ndf.model import NodeKind


class EvidenceAdapterTest(unittest.TestCase):
    def test_explicit_evidence_record_links_to_normative_clause(self) -> None:
        root = Path(__file__).parent / "fixtures" / "repository"
        project = ProjectManifest(
            format_version="0.2",
            project="fixture",
            roots=("evidence/*.json",),
            id_prefixes=("EVID",),
            domains=("core",),
            policies=(),
            root=root,
        )

        result = parse_evidence_manifest(root / "evidence/core.json", project)

        self.assertEqual(result.diagnostics, ())
        self.assertEqual(result.nodes[0].id.local_id, "EVID-CORE-001")
        self.assertEqual(result.nodes[0].kind, NodeKind.EVIDENCE)
        self.assertEqual(str(result.edges[0].source), "ndf://fixture/REQ-001")
        self.assertEqual(str(result.edges[0].target), "ndf://fixture/EVID-CORE-001")
        self.assertEqual(result.edges[0].type, "evidenced-by")

    def test_evidence_schema_requires_reproducibility_fields(self) -> None:
        schema = json.loads(
            (Path(__file__).parents[1] / "schemas/ndf-evidence.schema.json").read_text()
        )

        self.assertEqual(
            set(schema["required"]),
            {
                "format_version",
                "id",
                "command",
                "result",
                "tool_version",
                "source_revision",
                "timestamp",
                "links",
            },
        )


if __name__ == "__main__":
    unittest.main()
