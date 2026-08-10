import tempfile
import unittest
from pathlib import Path

from ndf.identity import NodeId
from ndf.index import open_index
from ndf.model import Edge, Graph, LifecycleStatus, Modality, Node, NodeKind, SourceSpan
from ndf.query import query_nodes, trace
from tests.index_test_support import build_fixture_index


class QueryAndTraceTest(unittest.TestCase):
    def test_query_filters_canonical_node_fields(self) -> None:
        requirement = self._node("REQ-001", NodeKind.REQUIREMENT, modality=Modality.MUST)
        information = self._node("INFO-001", NodeKind.INFORMATION)
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "graph.sqlite"
            build_fixture_index(Graph(nodes=(information, requirement)), output)

            with open_index(output) as index:
                result = query_nodes(index, "kind=requirement and status=active")

        self.assertEqual([str(node.id) for node in result], ["ndf://fixture/REQ-001"])

    def test_trace_terminates_on_cycle_and_reports_cycle_edge(self) -> None:
        first = self._node("REQ-001", NodeKind.REQUIREMENT)
        second = self._node("REQ-002", NodeKind.REQUIREMENT)
        graph = Graph(
            nodes=(first, second),
            edges=(
                Edge(first.id, second.id, "refines"),
                Edge(second.id, first.id, "refines"),
            ),
        )
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "graph.sqlite"
            build_fixture_index(graph, output)

            with open_index(output) as index:
                result = trace(index, "REQ-001", frozenset({"refines"}))

        self.assertEqual(result.root, "ndf://fixture/REQ-001")
        self.assertEqual(
            result.nodes,
            ("ndf://fixture/REQ-001", "ndf://fixture/REQ-002"),
        )
        self.assertEqual(len(result.cycles), 1)
        self.assertEqual(result.cycles[0].target, "ndf://fixture/REQ-001")

    @staticmethod
    def _node(
        local_id: str,
        kind: NodeKind,
        *,
        modality: Modality | None = None,
    ) -> Node:
        return Node(
            id=NodeId("fixture", local_id),
            kind=kind,
            title=local_id,
            source=SourceSpan("docs/design.md", 10),
            modality=modality,
            refinement="L1",
            status=LifecycleStatus.ACTIVE,
            owner="core",
            body="The core MUST preserve behavior." if modality else "Background.",
        )


if __name__ == "__main__":
    unittest.main()
