import sqlite3
import tempfile
import unittest
from pathlib import Path

from ndf.identity import NodeId
from ndf.index import BuildProvenance, build_index, open_index
from ndf.model import Edge, Graph, LifecycleStatus, Node, NodeKind, SourceSpan


class DeterministicIndexTest(unittest.TestCase):
    def test_reversed_insertion_order_has_identical_rows_and_hash(self) -> None:
        first, second = self._nodes()
        edge = Edge(second.id, first.id, "refines", second.source)
        forward = Graph(nodes=(first, second), edges=(edge,))
        reverse = Graph(nodes=(second, first), edges=(edge,))

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            first_build = build_index(
                forward,
                root / "forward.sqlite",
                self._provenance(root),
            )
            second_build = build_index(
                reverse,
                root / "reverse.sqlite",
                self._provenance(root),
            )

            self.assertEqual(first_build.semantic_hash, second_build.semantic_hash)
            self.assertEqual(
                self._logical_rows(root / "forward.sqlite"),
                self._logical_rows(root / "reverse.sqlite"),
            )

    def test_existing_output_is_replaced_only_after_successful_build(self) -> None:
        first, _ = self._nodes()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output = root / "graph.sqlite"
            output.write_bytes(b"original")
            incomplete = BuildProvenance(
                project_commits=(),
                format_version="0.2",
                ir_version="0.1",
                tool_version="0.1.0",
                normative_roots=(root / "docs",),
            )

            with self.assertRaisesRegex(ValueError, "missing project revision"):
                build_index(Graph(nodes=(first,)), output, incomplete)
            self.assertEqual(output.read_bytes(), b"original")

            build_index(Graph(nodes=(first,)), output, self._provenance(root))
            self.assertEqual(output.read_bytes()[:16], b"SQLite format 3\x00")

    def test_output_inside_normative_root_is_rejected(self) -> None:
        first, _ = self._nodes()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            normative_root = root / "docs"
            normative_root.mkdir()
            provenance = self._provenance(root, normative_roots=(normative_root,))

            with self.assertRaisesRegex(ValueError, "normative root"):
                build_index(
                    Graph(nodes=(first,)),
                    normative_root / "graph.sqlite",
                    provenance,
                )

    def test_open_index_enables_foreign_keys_and_exposes_graph_queries(self) -> None:
        first, second = self._nodes()
        graph = Graph(
            nodes=(first, second),
            edges=(Edge(second.id, first.id, "refines", second.source),),
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output = root / "graph.sqlite"
            build = build_index(graph, output, self._provenance(root))

            with open_index(output) as index:
                self.assertEqual(index.foreign_keys_enabled(), True)
                self.assertEqual(index.get_node(str(first.id)).title, "Contract")
                self.assertEqual(
                    [edge.type for edge in index.out_edges(str(second.id))],
                    ["refines"],
                )
                self.assertEqual(
                    [str(edge.source) for edge in index.in_edges(str(first.id))],
                    ["ndf://fixture/PIPE-020"],
                )
                self.assertEqual(
                    index.provenance(),
                    {
                        "format_version": "0.2",
                        "ir_version": "0.1",
                        "semantic_hash": build.semantic_hash,
                        "tool_version": "0.1.0",
                    },
                )

    @staticmethod
    def _nodes() -> tuple[Node, Node]:
        first = Node(
            id=NodeId("fixture", "PIPE-010"),
            kind=NodeKind.REQUIREMENT,
            title="Contract",
            source=SourceSpan("docs/pipeline.md", 10),
            status=LifecycleStatus.ACTIVE,
            owner="core",
            body="The core MUST issue in order.",
        )
        second = Node(
            id=NodeId("fixture", "PIPE-020"),
            kind=NodeKind.ARCHITECTURE,
            title="Mechanism",
            source=SourceSpan("docs/pipeline.md", 20),
            status=LifecycleStatus.ACTIVE,
            owner="core",
        )
        return first, second

    @staticmethod
    def _provenance(
        root: Path,
        *,
        normative_roots: tuple[Path, ...] | None = None,
    ) -> BuildProvenance:
        return BuildProvenance(
            project_commits=(
                ("fixture", "0123456789abcdef0123456789abcdef01234567"),
            ),
            format_version="0.2",
            ir_version="0.1",
            tool_version="0.1.0",
            normative_roots=normative_roots or (root / "docs",),
        )

    @staticmethod
    def _logical_rows(path: Path) -> dict[str, list[tuple]]:
        tables = (
            "projects",
            "sources",
            "nodes",
            "node_attrs",
            "aliases",
            "edges",
            "diagnostics",
            "baselines",
            "builds",
        )
        with sqlite3.connect(path) as connection:
            return {
                table: sorted(connection.execute(f"SELECT * FROM {table}").fetchall())
                for table in tables
            }


if __name__ == "__main__":
    unittest.main()
