import json
import tempfile
import unittest
from pathlib import Path

from ndf.export import ExportFormat, export_graph
from ndf.identity import NodeId
from ndf.index import open_index
from ndf.model import Graph, Node, NodeKind, SourceSpan
from tests.index_test_support import build_fixture_index


class GraphExportTest(unittest.TestCase):
    def test_ndf_json_export_is_deterministic_and_carries_provenance(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            node = Node(
                id=NodeId("fixture", "NODE-001"),
                kind=NodeKind.INFORMATION,
                title="Node",
                source=SourceSpan(str(root / "docs" / "node.md"), 1),
            )
            index_path = root / "graph.sqlite"
            first_path = root / "first.json"
            second_path = root / "second.json"
            build_fixture_index(Graph(nodes=(node,)), index_path)

            with open_index(index_path) as index:
                first = export_graph(index, first_path, ExportFormat.NDF_JSON)
                second = export_graph(index, second_path, ExportFormat.NDF_JSON)

            payload = json.loads(first_path.read_text())
            self.assertEqual(first_path.read_bytes(), second_path.read_bytes())
            self.assertEqual(first.semantic_hash, second.semantic_hash)
            self.assertEqual(payload["provenance"]["format_version"], "0.2")
            self.assertEqual(payload["nodes"][0]["id"], "ndf://fixture/NODE-001")
            self.assertNotIn(str(root), first_path.read_text())


if __name__ == "__main__":
    unittest.main()
