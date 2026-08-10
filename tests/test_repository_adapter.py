import unittest
from pathlib import Path

from ndf.adapters.repository import index_repository_paths
from ndf.manifest import ProjectManifest
from ndf.model import NodeKind


class RepositoryAdapterTest(unittest.TestCase):
    def setUp(self) -> None:
        self.root = Path(__file__).parent / "fixtures" / "repository"
        self.project = ProjectManifest(
            format_version="0.2",
            project="fixture",
            roots=("src/**/*.py", "tests/**/*.py"),
            id_prefixes=("REQ",),
            domains=("core",),
            policies=(),
            root=self.root,
        )

    def test_source_and_test_paths_get_deterministic_nodes_and_edges(self) -> None:
        forward = index_repository_paths(
            self.root,
            ("src/**/*.py", "tests/**/*.py"),
            self.project,
        )
        reverse = index_repository_paths(
            self.root,
            ("tests/**/*.py", "src/**/*.py"),
            self.project,
        )

        self.assertEqual(forward, reverse)
        self.assertEqual(
            [node.kind for node in forward.nodes],
            [NodeKind.SOURCE, NodeKind.TEST],
        )
        self.assertEqual(
            [(edge.type, edge.target.local_id) for edge in forward.edges],
            [("implements", "REQ-001"), ("verifies", "REQ-001")],
        )
        self.assertEqual(dict(forward.nodes[0].attributes)["path"], "src/core.py")
        self.assertEqual(len(dict(forward.nodes[0].attributes)["content_hash"]), 64)


if __name__ == "__main__":
    unittest.main()
