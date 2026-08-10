import json
import unittest
from pathlib import Path

from ndf.identity import NodeId
from ndf.model import (
    DiagnosticSeverity,
    Edge,
    Graph,
    LifecycleStatus,
    Node,
    NodeKind,
    SourceSpan,
)


class ModelInvariantTest(unittest.TestCase):
    def test_source_span_rejects_line_zero(self) -> None:
        with self.assertRaisesRegex(ValueError, "line must be positive"):
            SourceSpan("docs/pipeline.md", 0)

    def test_edge_rejects_empty_type(self) -> None:
        identity = NodeId("davincioo", "PIPE-001")
        with self.assertRaisesRegex(ValueError, "edge type must not be empty"):
            Edge(source=identity, target=identity, type="")

    def test_duplicate_node_emits_stable_diagnostic(self) -> None:
        identity = NodeId("davincioo", "PIPE-001")
        first = Node(
            id=identity,
            kind=NodeKind.REQUIREMENT,
            title="Issue contract",
            source=SourceSpan("docs/pipeline.md", 10),
            status=LifecycleStatus.ACTIVE,
        )
        duplicate = Node(
            id=identity,
            kind=NodeKind.REQUIREMENT,
            title="Duplicate issue contract",
            source=SourceSpan("docs/duplicate.md", 4),
            status=LifecycleStatus.ACTIVE,
        )

        graph = Graph().add_node(first).add_node(duplicate)

        self.assertEqual(len(graph.nodes), 1)
        self.assertEqual(len(graph.diagnostics), 1)
        self.assertEqual(graph.diagnostics[0].code, "NDF-ID-001")
        self.assertEqual(graph.diagnostics[0].severity, DiagnosticSeverity.ERROR)
        self.assertEqual(graph.diagnostics[0].primary, duplicate.source)
        self.assertEqual(graph.diagnostics[0].related, (first.source,))

    def test_graph_sorting_is_insertion_independent(self) -> None:
        later = Node(
            id=NodeId("davincioo", "PIPE-020"),
            kind=NodeKind.INFORMATION,
            title="Later",
            source=SourceSpan("docs/pipeline.md", 20),
        )
        earlier = Node(
            id=NodeId("davincioo", "PIPE-010"),
            kind=NodeKind.INFORMATION,
            title="Earlier",
            source=SourceSpan("docs/pipeline.md", 10),
        )

        graph = Graph().add_node(later).add_node(earlier)

        self.assertEqual(
            [str(node.id) for node in graph.sorted_nodes()],
            ["ndf://davincioo/PIPE-010", "ndf://davincioo/PIPE-020"],
        )

        graph = Graph(
            edges=(
                Edge(later.id, earlier.id, "refines"),
                Edge(earlier.id, later.id, "depends-on"),
            )
        )
        self.assertEqual(
            [edge.type for edge in graph.sorted_edges()],
            ["depends-on", "refines"],
        )

    def test_serialized_graph_matches_declared_schema_surface(self) -> None:
        graph = Graph().add_node(
            Node(
                id=NodeId("davincioo", "PIPE-001"),
                kind=NodeKind.REQUIREMENT,
                title="Issue contract",
                source=SourceSpan("docs/pipeline.md", 10),
                status=LifecycleStatus.ACTIVE,
            )
        )
        repository_root = Path(__file__).parents[1]
        graph_schema = json.loads(
            (repository_root / "schemas/ndf-ir.schema.json").read_text()
        )
        diagnostic_schema = json.loads(
            (repository_root / "schemas/ndf-diagnostic.schema.json").read_text()
        )

        self.assertEqual(
            set(graph.to_dict()),
            set(graph_schema["required"]),
        )
        self.assertEqual(
            set(Graph().add_node(graph.nodes[0]).add_node(graph.nodes[0]).diagnostics[0].to_dict()),
            set(diagnostic_schema["required"]),
        )


if __name__ == "__main__":
    unittest.main()
