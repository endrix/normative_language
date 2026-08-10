import re
import shlex
from pathlib import Path
from typing import Any

import yaml

from ndf.identity import NodeId, parse_uri
from ndf.manifest import ProjectManifest, _UniqueKeyLoader
from ndf.model import (
    Diagnostic,
    DiagnosticSeverity,
    Edge,
    Graph,
    LifecycleStatus,
    Modality,
    Node,
    NodeKind,
    ParseResult,
    SourceSpan,
)


_HEADING = re.compile(r"^(#{1,6})\s+(.+?)(?:\s+\{#([A-Za-z0-9._:-]+)\})?\s*$")
_FENCE = re.compile(r"^\s*(`{3,}|~{3,})")
_METADATA_PREFIX = "<!-- ndf:"
_EDGE_FIELDS = {
    "refines",
    "depends-on",
    "conflicts-with",
    "verifies",
    "derived-from",
    "implements",
    "affects",
    "blocks",
    "supersedes",
    "references",
    "evidenced-by",
}
_CANONICAL_FIELDS = {
    "kind",
    "modality",
    "refinement",
    "domain",
    "status",
    "owner",
} | _EDGE_FIELDS
_LEGACY_FIELDS = {"level", "layer"}


def _diagnostic(
    code: str,
    severity: DiagnosticSeverity,
    message: str,
    path: str,
    line: int,
    repair: str,
) -> Diagnostic:
    return Diagnostic(
        code=code,
        severity=severity,
        message=message,
        primary=SourceSpan(path, line),
        repair=repair,
    )


def _relative_path(path: Path, project: ProjectManifest) -> str:
    try:
        return path.resolve().relative_to(project.root.resolve()).as_posix()
    except ValueError as error:
        raise ValueError(f"document is outside project root: {path}") from error


def _front_matter(
    lines: list[str],
    relative_path: str,
) -> tuple[dict[str, Any] | None, int, tuple[Diagnostic, ...]]:
    if not lines or lines[0].strip() != "---":
        return (
            None,
            0,
            (
                _diagnostic(
                    "NDF-MD-001",
                    DiagnosticSeverity.ERROR,
                    "document front matter must start with ---",
                    relative_path,
                    1,
                    "Add complete YAML front matter at the start of the file.",
                ),
            ),
        )
    closing = next(
        (index for index, line in enumerate(lines[1:], start=1) if line.strip() == "---"),
        None,
    )
    if closing is None:
        return (
            None,
            0,
            (
                _diagnostic(
                    "NDF-MD-001",
                    DiagnosticSeverity.ERROR,
                    "document front matter is not closed",
                    relative_path,
                    1,
                    "Close the YAML front matter with ---.",
                ),
            ),
        )
    try:
        value = yaml.load("\n".join(lines[1:closing]), Loader=_UniqueKeyLoader)
    except (yaml.YAMLError, ValueError) as error:
        return (
            None,
            closing + 1,
            (
                _diagnostic(
                    "NDF-MD-002",
                    DiagnosticSeverity.ERROR,
                    f"invalid front matter: {error}",
                    relative_path,
                    1,
                    "Remove duplicate keys and repair the YAML mapping.",
                ),
            ),
        )
    if not isinstance(value, dict):
        return None, closing + 1, ()
    return value, closing + 1, ()


def _parse_metadata(
    line: str,
    relative_path: str,
    line_number: int,
) -> tuple[dict[str, str] | None, tuple[Diagnostic, ...]]:
    stripped = line.strip()
    if stripped.startswith(_METADATA_PREFIX) and not stripped.endswith("-->"):
        return (
            None,
            (
                _diagnostic(
                    "NDF-MD-001",
                    DiagnosticSeverity.ERROR,
                    "NDF metadata comment is not closed",
                    relative_path,
                    line_number,
                    "Close the metadata comment with --> on the same line.",
                ),
            ),
        )
    if not (stripped.startswith(_METADATA_PREFIX) and stripped.endswith("-->")):
        return None, ()

    content = stripped[len(_METADATA_PREFIX) : -3].strip()
    try:
        tokens = shlex.split(content)
    except ValueError as error:
        return (
            None,
            (
                _diagnostic(
                    "NDF-MD-001",
                    DiagnosticSeverity.ERROR,
                    f"invalid NDF metadata delimiter: {error}",
                    relative_path,
                    line_number,
                    "Repair quoting and keep metadata on one line.",
                ),
            ),
        )

    metadata: dict[str, str] = {}
    for token in tokens:
        if "=" not in token:
            return (
                None,
                (
                    _diagnostic(
                        "NDF-MD-001",
                        DiagnosticSeverity.ERROR,
                        f"invalid NDF metadata token: {token}",
                        relative_path,
                        line_number,
                        "Write metadata as key=value tokens.",
                    ),
                ),
            )
        key, value = token.split("=", 1)
        if key in metadata:
            return (
                None,
                (
                    _diagnostic(
                        "NDF-MD-002",
                        DiagnosticSeverity.ERROR,
                        f"duplicate NDF metadata key: {key}",
                        relative_path,
                        line_number,
                        "Keep exactly one value for each metadata key.",
                    ),
                ),
            )
        metadata[key] = value

    unknown = sorted(set(metadata) - _CANONICAL_FIELDS - _LEGACY_FIELDS)
    if unknown:
        return (
            None,
            (
                _diagnostic(
                    "NDF-MD-003",
                    DiagnosticSeverity.ERROR,
                    f"unsupported NDF metadata field: {unknown[0]}",
                    relative_path,
                    line_number,
                    "Remove the field or register it in the project format.",
                ),
            ),
        )
    return metadata, ()


def _normalize_metadata(
    metadata: dict[str, str],
    relative_path: str,
    line_number: int,
) -> tuple[dict[str, str], tuple[Diagnostic, ...]]:
    normalized = dict(metadata)
    migrated = False
    if normalized.get("kind") == "req":
        normalized["kind"] = "requirement"
        migrated = True
    if "level" in normalized:
        normalized["modality"] = normalized.pop("level")
        migrated = True
    if "layer" in normalized:
        normalized["refinement"] = normalized.pop("layer")
        migrated = True
    if normalized.get("status") == "stable":
        normalized["status"] = "active"
        migrated = True
    if not migrated:
        return normalized, ()
    return (
        normalized,
        (
            _diagnostic(
                "NDF-MIG-001",
                DiagnosticSeverity.INFORMATION,
                "legacy DavinciOO metadata was mapped to canonical fields",
                relative_path,
                line_number,
                "Rewrite the clause with kind, modality, refinement, and status.",
            ),
        ),
    )


def _body(lines: list[str], start: int) -> str:
    collected: list[str] = []
    fence: str | None = None
    for line in lines[start:]:
        fence_match = _FENCE.match(line)
        if fence_match:
            marker = fence_match.group(1)
            if fence is None:
                fence = marker[0]
            elif marker[0] == fence:
                fence = None
            collected.append(line)
            continue
        if fence is None and _HEADING.match(line):
            break
        collected.append(line)
    return "\n".join(collected).strip()


def parse_markdown(path: Path, project: ProjectManifest) -> ParseResult:
    relative_path = _relative_path(path, project)
    lines = path.read_text().splitlines()
    front_matter, content_start, front_diagnostics = _front_matter(lines, relative_path)
    if front_matter is None:
        return ParseResult(diagnostics=front_diagnostics)

    required_front = {"doc_id", "status", "authority", "owner"}
    missing_front = sorted(required_front - set(front_matter))
    if missing_front:
        return ParseResult(
            diagnostics=(
                _diagnostic(
                    "NDF-MD-003",
                    DiagnosticSeverity.ERROR,
                    f"missing front matter field: {missing_front[0]}",
                    relative_path,
                    1,
                    "Add all required document front matter fields.",
                ),
            )
        )
    unknown_front = sorted(set(front_matter) - required_front)
    if unknown_front:
        return ParseResult(
            diagnostics=(
                _diagnostic(
                    "NDF-MD-003",
                    DiagnosticSeverity.ERROR,
                    f"unsupported front matter field: {unknown_front[0]}",
                    relative_path,
                    1,
                    "Remove unsupported document front matter.",
                ),
            )
        )

    title = str(front_matter["doc_id"])
    fence: str | None = None
    for line in lines[content_start:]:
        fence_match = _FENCE.match(line)
        if fence_match:
            marker = fence_match.group(1)
            if fence is None:
                fence = marker[0]
            elif marker[0] == fence:
                fence = None
            continue
        heading = _HEADING.match(line)
        if fence is None and heading and len(heading.group(1)) == 1:
            title = heading.group(2)
            break

    try:
        document_status = LifecycleStatus(str(front_matter["status"]))
    except ValueError:
        return ParseResult(
            diagnostics=(
                _diagnostic(
                    "NDF-MD-004",
                    DiagnosticSeverity.ERROR,
                    f"invalid document status: {front_matter['status']}",
                    relative_path,
                    1,
                    "Use a canonical lifecycle status.",
                ),
            )
        )

    document = Node(
        id=NodeId(project.project, str(front_matter["doc_id"])),
        kind=NodeKind.DOCUMENT,
        title=title,
        source=SourceSpan(relative_path, 1),
        status=document_status,
        owner=str(front_matter["owner"]),
        attributes=(("authority", str(front_matter["authority"])),),
    )
    graph = Graph().add_node(document)
    edges: list[Edge] = []
    diagnostics: list[Diagnostic] = list(front_diagnostics)
    fence = None

    for index, line in enumerate(lines[content_start:], start=content_start):
        fence_match = _FENCE.match(line)
        if fence_match:
            marker = fence_match.group(1)
            if fence is None:
                fence = marker[0]
            elif marker[0] == fence:
                fence = None
            continue
        if fence is not None:
            continue
        heading = _HEADING.match(line)
        if not heading or not heading.group(3):
            continue
        heading_line = index + 1
        metadata_index = index + 1
        if metadata_index >= len(lines):
            continue
        metadata, metadata_diagnostics = _parse_metadata(
            lines[metadata_index],
            relative_path,
            metadata_index + 1,
        )
        diagnostics.extend(metadata_diagnostics)
        if metadata is None:
            continue
        normalized, migration_diagnostics = _normalize_metadata(
            metadata,
            relative_path,
            metadata_index + 1,
        )
        diagnostics.extend(migration_diagnostics)
        try:
            node = Node(
                id=NodeId(project.project, heading.group(3)),
                kind=NodeKind(normalized.get("kind", "information")),
                title=heading.group(2),
                source=SourceSpan(relative_path, heading_line),
                modality=(
                    Modality(normalized["modality"])
                    if "modality" in normalized
                    else None
                ),
                refinement=normalized.get("refinement"),
                domain=normalized.get("domain"),
                status=LifecycleStatus(normalized.get("status", "draft")),
                owner=normalized.get("owner", str(front_matter["owner"])),
                body=_body(lines, metadata_index + 1),
            )
        except ValueError as error:
            diagnostics.append(
                _diagnostic(
                    "NDF-MD-004",
                    DiagnosticSeverity.ERROR,
                    f"invalid canonical metadata value: {error}",
                    relative_path,
                    metadata_index + 1,
                    "Use canonical kind, modality, and lifecycle values.",
                )
            )
            continue
        graph = graph.add_node(node)
        for edge_type in sorted(_EDGE_FIELDS):
            if edge_type not in normalized:
                continue
            for target in normalized[edge_type].split(","):
                edges.append(
                    Edge(
                        source=node.id,
                        target=parse_uri(target.strip(), current_project=project.project),
                        type=edge_type,
                        source_span=SourceSpan(relative_path, metadata_index + 1),
                    )
                )

    return ParseResult(
        nodes=graph.nodes,
        edges=tuple(edges),
        diagnostics=graph.diagnostics + tuple(diagnostics),
    )
