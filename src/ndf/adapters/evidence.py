import json
from pathlib import Path

from ndf.identity import NodeId, parse_uri
from ndf.manifest import ProjectManifest
from ndf.model import (
    Diagnostic,
    DiagnosticSeverity,
    Edge,
    LifecycleStatus,
    Node,
    NodeKind,
    ParseResult,
    SourceSpan,
)


_REQUIRED = {
    "format_version",
    "id",
    "command",
    "result",
    "tool_version",
    "source_revision",
    "timestamp",
    "links",
}


def parse_evidence_manifest(path: Path, project: ProjectManifest) -> ParseResult:
    relative = path.resolve().relative_to(project.root.resolve()).as_posix()
    try:
        record = json.loads(path.read_text())
    except json.JSONDecodeError as error:
        return ParseResult(
            diagnostics=(
                Diagnostic(
                    "NDF-EVID-001",
                    DiagnosticSeverity.ERROR,
                    f"invalid evidence JSON: {error}",
                    SourceSpan(relative, error.lineno),
                    repair="Repair the JSON evidence manifest.",
                ),
            )
        )
    if not isinstance(record, dict) or set(record) != _REQUIRED:
        return ParseResult(
            diagnostics=(
                Diagnostic(
                    "NDF-EVID-001",
                    DiagnosticSeverity.ERROR,
                    "evidence manifest does not match the required fields",
                    SourceSpan(relative, 1),
                    repair="Provide every reproducibility field and no unknown fields.",
                ),
            )
        )
    identity = NodeId(project.project, str(record["id"]))
    attributes = tuple(
        sorted(
            (key, str(record[key]))
            for key in (
                "command",
                "result",
                "tool_version",
                "source_revision",
                "timestamp",
            )
        )
    )
    node = Node(
        id=identity,
        kind=NodeKind.EVIDENCE,
        title=str(record["command"]),
        source=SourceSpan(relative, 1),
        status=LifecycleStatus.ACTIVE,
        owner=project.project,
        attributes=attributes,
    )
    edges = tuple(
        Edge(
            source=parse_uri(link["clause"], current_project=project.project),
            target=identity,
            type=link["type"],
            source_span=SourceSpan(relative, 1),
        )
        for link in record["links"]
    )
    return ParseResult((node,), edges, ())
