import unittest
from pathlib import Path

from ndf.manifest import ProjectManifest
from ndf.model import LifecycleStatus, NodeKind
from ndf.adapters.pto_asl import instruction_clause_id, parse_pto_asl


class PtoAslAdapterTest(unittest.TestCase):
    def setUp(self) -> None:
        self.fixture_root = Path(__file__).parent / "fixtures" / "pto-asl"
        self.project = ProjectManifest(
            format_version="0.2",
            project="pto-spec",
            roots=("*.asl",),
            id_prefixes=("PTO",),
            domains=(
                "architecture",
                "scalar",
                "block",
                "tile",
                "state",
                "memory",
                "concurrency",
            ),
            policies=(),
            root=self.fixture_root,
        )

    def test_valid_regions_and_instruction_metadata_become_canonical_graph(self) -> None:
        result = parse_pto_asl(self.fixture_root / "accept.asl", self.project)

        self.assertEqual(result.diagnostics, ())
        self.assertEqual(
            [node.id.local_id for node in result.nodes],
            [
                "PTO-INST-TILE-TLOAD",
                "PTO-TILE-CAPACITY",
                "PTO-TILE-CAPACITY-MECHANISM",
            ],
        )
        contract = result.nodes[1]
        mechanism = result.nodes[2]
        self.assertEqual(contract.kind, NodeKind.REQUIREMENT)
        self.assertEqual(contract.refinement, "L1")
        self.assertEqual(contract.domain, "tile")
        self.assertEqual(contract.status, LifecycleStatus.ACTIVE)
        self.assertEqual(contract.source.line, 3)
        self.assertEqual(mechanism.kind, NodeKind.ARCHITECTURE)
        self.assertEqual(mechanism.status, LifecycleStatus.OPEN)
        self.assertEqual(
            [(edge.type, edge.target.local_id) for edge in result.edges],
            [("references", "PTO-INST-TILE-TLOAD")],
        )

    def test_non_comment_body_line_is_rejected(self) -> None:
        path = self.fixture_root / "accept.asl"
        text = path.read_text().replace(
            "// The implementation tracks capacity in tile state.",
            "let capacity = 1;",
        )

        result = parse_pto_asl_text(text, path, self.project)

        self.assertIn("NDF-ASL-003", [item.code for item in result.diagnostics])

    def test_duplicate_metadata_field_is_rejected(self) -> None:
        result = parse_pto_asl(
            self.fixture_root / "reject-duplicate.asl",
            self.project,
        )

        self.assertEqual([item.code for item in result.diagnostics], ["NDF-ASL-002"])

    def test_kind_level_layer_and_status_are_validated(self) -> None:
        result = parse_pto_asl(
            self.fixture_root / "reject-metadata.asl",
            self.project,
        )

        self.assertEqual(
            [item.code for item in result.diagnostics],
            ["NDF-ASL-002", "NDF-ASL-002", "NDF-ASL-002"],
        )

    def test_mismatched_region_end_is_rejected(self) -> None:
        path = self.fixture_root / "accept.asl"
        text = path.read_text().replace(
            "NDF-END: PTO-TILE-CAPACITY",
            "NDF-END: PTO-OTHER",
            1,
        )

        result = parse_pto_asl_text(text, path, self.project)

        self.assertIn("NDF-ASL-001", [item.code for item in result.diagnostics])

    def test_instruction_identity_is_surface_scoped_and_stable(self) -> None:
        self.assertEqual(
            instruction_clause_id("tile", "TLOAD"),
            "PTO-INST-TILE-TLOAD",
        )
        self.assertEqual(
            instruction_clause_id("block", "BSTART.TLOAD"),
            "PTO-INST-BLOCK-BSTART-TLOAD",
        )


def parse_pto_asl_text(
    text: str,
    source: Path,
    project: ProjectManifest,
):
    from ndf.adapters.pto_asl import parse_pto_asl_text as parse

    return parse(text, source, project)


if __name__ == "__main__":
    unittest.main()
