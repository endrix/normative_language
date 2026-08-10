import tempfile
import unittest
from pathlib import Path

from ndf.diff import semantic_diff
from ndf.identity import NodeId
from ndf.index import open_index
from ndf.model import Edge, Graph, LifecycleStatus, Node, NodeKind, SourceSpan
from tests.index_test_support import build_fixture_index


class SemanticDiffTest(unittest.TestCase):
    def test_classifies_node_and_edge_changes(self) -> None:
        before_nodes = (
            self._node("MOVED", "Moved", "docs/old.md"),
            self._node("MODIFIED", "Before", "docs/design.md"),
            self._node("REMOVED", "Removed", "docs/design.md"),
            self._node("SUPERSEDED", "Superseded", "docs/design.md"),
            self._node("EDGE-A", "Edge A", "docs/design.md"),
            self._node("EDGE-B", "Edge B", "docs/design.md"),
        )
        after_nodes = (
            self._node("MOVED", "Moved", "docs/new.md"),
            self._node("MODIFIED", "After", "docs/design.md"),
            self._node("ADDED", "Added", "docs/design.md"),
            self._node(
                "SUPERSEDED",
                "Superseded",
                "docs/design.md",
                LifecycleStatus.SUPERSEDED,
            ),
            self._node("EDGE-A", "Edge A", "docs/design.md"),
            self._node("EDGE-B", "Edge B", "docs/design.md"),
        )
        after_edges = (
            Edge(
                NodeId("fixture", "EDGE-A"),
                NodeId("fixture", "EDGE-B"),
                "depends-on",
            ),
        )

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            before_path = root / "before.sqlite"
            after_path = root / "after.sqlite"
            build_fixture_index(Graph(nodes=before_nodes), before_path)
            build_fixture_index(Graph(nodes=after_nodes, edges=after_edges), after_path)

            with open_index(before_path) as before, open_index(after_path) as after:
                result = semantic_diff(before, after)

        self.assertEqual(result.added, ("ndf://fixture/ADDED",))
        self.assertEqual(result.removed, ("ndf://fixture/REMOVED",))
        self.assertEqual(result.modified, ("ndf://fixture/MODIFIED",))
        self.assertEqual(result.moved, ("ndf://fixture/MOVED",))
        self.assertEqual(result.superseded, ("ndf://fixture/SUPERSEDED",))
        self.assertEqual(
            result.edge_only,
            ("ndf://fixture/EDGE-A", "ndf://fixture/EDGE-B"),
        )
        self.assertEqual(
            result.edges_added,
            (("ndf://fixture/EDGE-A", "depends-on", "ndf://fixture/EDGE-B"),),
        )

    @staticmethod
    def _node(
        local_id: str,
        title: str,
        path: str,
        status: LifecycleStatus = LifecycleStatus.ACTIVE,
    ) -> Node:
        return Node(
            id=NodeId("fixture", local_id),
            kind=NodeKind.ARCHITECTURE,
            title=title,
            source=SourceSpan(path, 10),
            status=status,
            owner="core",
            body=f"Body for {local_id}",
        )


if __name__ == "__main__":
    unittest.main()
