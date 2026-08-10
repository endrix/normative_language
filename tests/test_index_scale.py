import os
import tempfile
import time
import unittest
from pathlib import Path

from ndf.identity import NodeId
from ndf.index import BuildProvenance, build_index
from ndf.model import Edge, Graph, Node, NodeKind, SourceSpan


@unittest.skipUnless(
    os.environ.get("NDF_RUN_SCALE_TESTS") == "1",
    "set NDF_RUN_SCALE_TESTS=1 to run the provisional scale measurement",
)
class ProvisionalIndexScaleTest(unittest.TestCase):
    def test_builds_one_hundred_thousand_nodes_and_one_million_edges(self) -> None:
        nodes = tuple(
            Node(
                id=NodeId("scale", f"NODE-{index:06d}"),
                kind=NodeKind.INFORMATION,
                title=f"Node {index}",
                source=SourceSpan("generated/scale.md", index + 1),
            )
            for index in range(100_000)
        )
        edges = tuple(
            Edge(
                source=nodes[index % len(nodes)].id,
                target=nodes[(index + 1) % len(nodes)].id,
                type="references",
            )
            for index in range(1_000_000)
        )
        provenance = BuildProvenance(
            project_commits=(
                ("scale", "0123456789abcdef0123456789abcdef01234567"),
            ),
            format_version="0.2",
            ir_version="0.1",
            tool_version="0.1.0",
        )

        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "scale.sqlite"
            started = time.monotonic()
            build = build_index(Graph(nodes=nodes, edges=edges), output, provenance)
            elapsed = time.monotonic() - started

            self.assertEqual(build.node_count, 100_000)
            self.assertEqual(build.edge_count, 1_000_000)
            print(f"NDF scale build: {elapsed:.3f}s, {output.stat().st_size} bytes")


if __name__ == "__main__":
    unittest.main()
