from __future__ import annotations

import argparse
from contextlib import redirect_stdout
from io import StringIO
import json
from pathlib import Path
import sqlite3
import sys
import tempfile
from typing import Any, Callable


REPOSITORY = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPOSITORY / "src"))
sys.path.insert(0, str(Path(__file__).resolve().parent))

from normalize import normalized_json
from ndf.adapters.pto_asl import parse_pto_asl
from ndf.cli import main
from ndf.identity import NodeId, canonical_uri, parse_uri
from ndf.index import BuildProvenance, build_index, open_index
from ndf.manifest import ProjectManifest, load_manifest
from ndf.model import (
    Edge,
    Graph,
    LifecycleStatus,
    Node,
    NodeKind,
    SourceSpan,
)
from ndf.parsers.markdown import parse_markdown
from ndf.query import query_nodes, trace
from ndf.report import CoveragePolicy, coverage


GOLDEN = REPOSITORY / "tests" / "golden"
FIXTURES = REPOSITORY / "tests" / "fixtures"
REVISION = "1" * 40
TABLES = (
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


def graph_dict(result: Any) -> dict[str, Any]:
    return Graph(result.nodes, result.edges, result.diagnostics).to_dict()


def machine_version() -> dict[str, Any]:
    output = StringIO()
    with redirect_stdout(output):
        status = main(["--machine-version"])
    return {"exit_code": status, "payload": json.loads(output.getvalue())}


def captured_error(operation: Callable[[], object]) -> str:
    try:
        operation()
    except ValueError as error:
        return str(error)
    raise AssertionError("operation did not raise ValueError")


def identity_contract() -> dict[str, Any]:
    local = parse_uri("VEC-ISSUE-001", current_project="davincioo")
    canonical = parse_uri("ndf://pto-spec/PTO-INST-TLOAD")
    return {
        "local": str(local),
        "canonical": {
            "project_id": canonical.project_id,
            "local_id": canonical.local_id,
            "uri": canonical_uri(canonical.project_id, canonical.local_id),
        },
        "errors": {
            "missing_project": captured_error(lambda: parse_uri("VEC-ISSUE-001")),
            "invalid_project": captured_error(
                lambda: canonical_uri("DavinciOO", "PIPE-001")
            ),
            "invalid_uri": captured_error(
                lambda: parse_uri("ndf://pto-spec/path/with/slash")
            ),
        },
    }


def markdown_valid() -> dict[str, Any]:
    root = FIXTURES / "markdown" / "project"
    manifest = load_manifest(root / "ndf.yaml")
    return graph_dict(parse_markdown(root / "docs" / "pipeline.md", manifest))


def markdown_invalid() -> dict[str, Any]:
    root = FIXTURES / "oracle" / "markdown-invalid"
    manifest = ProjectManifest(
        format_version="0.2",
        project="fixture",
        roots=("*.md",),
        id_prefixes=("DOC", "PIPE"),
        domains=("core",),
        policies=(),
        root=root,
    )
    return {
        path.name: graph_dict(parse_markdown(path, manifest))
        for path in sorted(root.glob("*.md"))
    }


def pto_asl() -> dict[str, Any]:
    root = FIXTURES / "pto-asl"
    project = ProjectManifest(
        format_version="0.2",
        project="pto-spec",
        roots=("*.asl",),
        id_prefixes=("PTO",),
        domains=(
            "architecture",
            "scalar",
            "block",
            "tile",
            "state",
            "memory",
            "concurrency",
        ),
        policies=(),
        root=root,
    )
    return {
        path.name: graph_dict(parse_pto_asl(path, project))
        for path in sorted(root.glob("*.asl"))
    }


def index_graph() -> Graph:
    contract = Node(
        id=NodeId("fixture", "PIPE-010"),
        kind=NodeKind.REQUIREMENT,
        title="Contract",
        source=SourceSpan("docs/pipeline.md", 10),
        status=LifecycleStatus.ACTIVE,
        owner="core",
        body="The core MUST issue in order.",
    )
    mechanism = Node(
        id=NodeId("fixture", "PIPE-020"),
        kind=NodeKind.ARCHITECTURE,
        title="Mechanism",
        source=SourceSpan("docs/pipeline.md", 20),
        status=LifecycleStatus.ACTIVE,
        owner="core",
    )
    return Graph(
        nodes=(mechanism, contract),
        edges=(Edge(mechanism.id, contract.id, "refines", mechanism.source),),
    )


def provenance() -> BuildProvenance:
    return BuildProvenance(
        project_commits=(("fixture", REVISION),),
        format_version="0.2",
        ir_version="0.1",
        tool_version="0.1.0",
    )


def logical_rows(path: Path) -> dict[str, list[list[Any]]]:
    with sqlite3.connect(path) as connection:
        return {
            table: [list(row) for row in sorted(connection.execute(f"SELECT * FROM {table}"))]
            for table in TABLES
        }


def index_contract() -> dict[str, Any]:
    with tempfile.TemporaryDirectory() as directory:
        output = Path(directory) / "graph.sqlite"
        result = build_index(index_graph(), output, provenance())
        return {
            "semantic_hash": result.semantic_hash,
            "node_count": result.node_count,
            "edge_count": result.edge_count,
            "rows": logical_rows(output),
        }


def query_report() -> dict[str, Any]:
    root = FIXTURES / "markdown" / "project"
    manifest = load_manifest(root / "ndf.yaml")
    parsed = parse_markdown(root / "docs" / "pipeline.md", manifest)
    graph = Graph(parsed.nodes, parsed.edges, parsed.diagnostics)
    build_provenance = BuildProvenance(
        project_commits=(("fixture", REVISION),),
        format_version="0.2",
        ir_version="0.1",
        tool_version="0.1.0",
    )
    with tempfile.TemporaryDirectory() as directory:
        output = Path(directory) / "graph.sqlite"
        build_index(graph, output, build_provenance)
        with open_index(output) as index:
            return {
                "query": [
                    node.to_dict()
                    for node in query_nodes(
                        index,
                        "kind=requirement and status=active",
                    )
                ],
                "trace": trace(
                    index,
                    "PIPE-ISSUE-010",
                    frozenset({"refines"}),
                ).to_dict(),
                "coverage": coverage(index, CoveragePolicy()).to_dict(),
            }


GENERATORS: dict[str, Callable[[], dict[str, Any]]] = {
    "machine-version.json": machine_version,
    "identity.json": identity_contract,
    "markdown-valid.json": markdown_valid,
    "markdown-invalid.json": markdown_invalid,
    "pto-asl.json": pto_asl,
    "index.json": index_contract,
    "query-report.json": query_report,
}


def run(check: bool) -> int:
    mismatches: list[str] = []
    for name, generator in GENERATORS.items():
        expected = normalized_json(generator(), REPOSITORY)
        path = GOLDEN / name
        if check:
            if not path.exists() or path.read_text() != expected:
                mismatches.append(name)
            continue
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(expected)
    if mismatches:
        print("golden mismatch: " + ", ".join(mismatches))
        return 1
    return 0


def main_entry() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    return run(parser.parse_args().check)


if __name__ == "__main__":
    raise SystemExit(main_entry())
