import unittest
from pathlib import Path

from ndf.identity import NodeId
from ndf.manifest import ProjectManifest
from ndf.model import Edge, Graph, LifecycleStatus, Modality, Node, NodeKind, SourceSpan
from ndf.plugins import Rule, load_project_rules
from ndf.rules import ValidationPolicy, validate_graph


class GraphRuleTest(unittest.TestCase):
    def test_duplicate_identity_is_preserved_as_validation_error(self) -> None:
        node = self._requirement("PIPE-001")
        graph = Graph().add_node(node).add_node(node)

        self.assertEqual(self._codes(graph), ["NDF-ID-001"])

    def test_dangling_mandatory_edge_is_rejected(self) -> None:
        source = self._requirement("PIPE-001")
        graph = Graph(
            nodes=(source,),
            edges=(
                Edge(source.id, NodeId("fixture", "PIPE-MISSING"), "refines"),
            ),
        )

        self.assertEqual(self._codes(graph), ["NDF-EDGE-001"])

    def test_must_requirement_without_normative_keyword_is_rejected(self) -> None:
        node = self._requirement("PIPE-001", body="The pipeline issues in order.")

        self.assertEqual(self._codes(Graph(nodes=(node,))), ["NDF-MOD-001"])

    def test_information_text_cannot_create_normative_requirement(self) -> None:
        node = Node(
            id=NodeId("fixture", "PIPE-INFO-001"),
            kind=NodeKind.INFORMATION,
            title="Background",
            source=SourceSpan("docs/pipeline.md", 20),
            body="The implementation MUST issue in order.",
        )

        self.assertEqual(self._codes(Graph(nodes=(node,))), ["NDF-MOD-002"])

    def test_lifecycle_cannot_move_from_active_back_to_draft(self) -> None:
        node = self._requirement(
            "PIPE-001",
            status=LifecycleStatus.DRAFT,
            attributes=(("previous_status", "active"),),
        )

        self.assertEqual(self._codes(Graph(nodes=(node,))), ["NDF-LIFE-001"])

    def test_explicit_normative_node_requires_owner(self) -> None:
        node = self._requirement("PIPE-001", owner=None)

        self.assertEqual(self._codes(Graph(nodes=(node,))), ["NDF-OWN-001"])

    def test_known_project_rules_are_declarative(self) -> None:
        manifest = ProjectManifest(
            format_version="0.2",
            project="fixture",
            roots=("docs/**/*.md",),
            id_prefixes=("PIPE",),
            domains=("core",),
            policies=(("ndf", {"ownership_required": True}),),
            root=Path.cwd(),
        )

        self.assertEqual(
            load_project_rules(manifest),
            (Rule("ndf", "ownership_required", True),),
        )

    def test_unknown_rule_namespace_is_rejected(self) -> None:
        manifest = ProjectManifest(
            format_version="0.2",
            project="fixture",
            roots=("docs/**/*.md",),
            id_prefixes=("PIPE",),
            domains=("core",),
            policies=(("python.module", {"rule": "load_me"}),),
            root=Path.cwd(),
        )

        with self.assertRaisesRegex(ValueError, "NDF-PLUGIN-001"):
            load_project_rules(manifest)

    def _codes(self, graph: Graph) -> list[str]:
        return [
            diagnostic.code
            for diagnostic in validate_graph(graph, ValidationPolicy())
        ]

    @staticmethod
    def _requirement(
        local_id: str,
        *,
        body: str = "The pipeline MUST issue in order.",
        owner: str | None = "core",
        status: LifecycleStatus = LifecycleStatus.ACTIVE,
        attributes: tuple[tuple[str, str], ...] = (),
    ) -> Node:
        return Node(
            id=NodeId("fixture", local_id),
            kind=NodeKind.REQUIREMENT,
            title="Issue contract",
            source=SourceSpan("docs/pipeline.md", 10),
            modality=Modality.MUST,
            status=status,
            owner=owner,
            body=body,
            attributes=attributes,
        )


if __name__ == "__main__":
    unittest.main()
