import tempfile
import unittest
from pathlib import Path

from ndf.identity import NodeId
from ndf.index import open_index
from ndf.model import Edge, Graph, LifecycleStatus, Modality, Node, NodeKind, SourceSpan
from ndf.report import CoveragePolicy, coverage
from tests.index_test_support import build_fixture_index


class CoverageReportTest(unittest.TestCase):
    def test_reports_unverified_open_and_tbd_contracts(self) -> None:
        covered = self._node("REQ-COVERED", NodeKind.REQUIREMENT, Modality.MUST, "L1")
        uncovered = self._node("REQ-UNCOVERED", NodeKind.REQUIREMENT, Modality.MUST, "L1")
        verification = self._node("VER-001", NodeKind.VERIFICATION, None, "L2")
        question = self._node("QUESTION-001", NodeKind.QUESTION, None, "L0", LifecycleStatus.OPEN)
        option = self._node("OPTION-001", NodeKind.OPTION, None, "L0", LifecycleStatus.OPEN)
        tbd = self._node("REQ-TBD", NodeKind.REQUIREMENT, Modality.TBD, "L1")
        graph = Graph(
            nodes=(covered, uncovered, verification, question, option, tbd),
            edges=(Edge(verification.id, covered.id, "verifies"),),
        )
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "graph.sqlite"
            build_fixture_index(graph, output)

            with open_index(output) as index:
                report = coverage(index, CoveragePolicy(refinement="L1"))

        self.assertEqual(report.unverified, ("ndf://fixture/REQ-UNCOVERED",))
        self.assertEqual(
            report.open_items,
            ("ndf://fixture/OPTION-001", "ndf://fixture/QUESTION-001"),
        )
        self.assertEqual(report.tbd, ("ndf://fixture/REQ-TBD",))

    @staticmethod
    def _node(
        local_id: str,
        kind: NodeKind,
        modality: Modality | None,
        refinement: str,
        status: LifecycleStatus = LifecycleStatus.ACTIVE,
    ) -> Node:
        return Node(
            id=NodeId("fixture", local_id),
            kind=kind,
            title=local_id,
            source=SourceSpan("docs/design.md", 10),
            modality=modality,
            refinement=refinement,
            status=status,
            owner="core",
            body="The core MUST preserve behavior." if modality is Modality.MUST else "",
        )


if __name__ == "__main__":
    unittest.main()
