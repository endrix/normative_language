from pathlib import Path

from ndf.index import BuildProvenance, build_index
from ndf.model import Graph


def build_fixture_index(graph: Graph, output: Path) -> None:
    projects = sorted({node.id.project_id for node in graph.nodes})
    provenance = BuildProvenance(
        project_commits=tuple((project, "0" * 40) for project in projects),
        format_version="0.2",
        ir_version="0.1",
        tool_version="0.1.0",
    )
    build_index(graph, output, provenance)
