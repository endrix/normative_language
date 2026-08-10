import json
import re
from pathlib import Path

from ndf.identity import NodeId
from ndf.manifest import ProjectManifest
from ndf.model import (
    Diagnostic,
    DiagnosticSeverity,
    Edge,
    Graph,
    LifecycleStatus,
    Node,
    NodeKind,
    ParseResult,
    SourceSpan,
)


_CLAUSE_ID = re.compile(r"PTO-[A-Z0-9]+(?:-[A-Z0-9]+)*")
_REGION_BEGIN = re.compile(r"^// NDF-BEGIN: (PTO-[A-Z0-9]+(?:-[A-Z0-9]+)*)$")
_REGION_END = re.compile(r"^// NDF-END: (PTO-[A-Z0-9]+(?:-[A-Z0-9]+)*)$")
_REFERENCE = re.compile(r"\[\[(PTO-[A-Z0-9]+(?:-[A-Z0-9]+)*)\]\]")
_METADATA_PREFIX = "// ndf: "
_INSTRUCTION_PREFIX = "// PTO-INSTRUCTION: "
_KINDS = {
    "intent": ("L0", NodeKind.ARCHITECTURE),
    "contract": ("L1", NodeKind.REQUIREMENT),
    "mechanism": ("L2", NodeKind.ARCHITECTURE),
    "executable": ("L3", NodeKind.MODEL),
}
_LAYERS = {
    "architecture",
    "scalar",
    "block",
    "tile",
    "state",
    "memory",
    "concurrency",
}
_STATUSES = {
    "open": LifecycleStatus.OPEN,
    "accepted": LifecycleStatus.ACTIVE,
}


def instruction_clause_id(surface: str, mnemonic: str) -> str:
    surface_slug = re.sub(r"[^A-Z0-9]+", "-", surface.upper()).strip("-")
    mnemonic_slug = re.sub(r"[^A-Z0-9]+", "-", mnemonic.upper()).strip("-")
    if not surface_slug or not mnemonic_slug:
        raise ValueError("instruction NDF identity requires surface and mnemonic")
    return f"PTO-INST-{surface_slug}-{mnemonic_slug}"


def _relative_path(source: Path, project: ProjectManifest) -> str:
    return source.resolve().relative_to(project.root.resolve()).as_posix()


def _diagnostic(
    code: str,
    message: str,
    path: str,
    line: int,
    repair: str,
    subject: NodeId | None = None,
) -> Diagnostic:
    return Diagnostic(
        code=code,
        severity=DiagnosticSeverity.ERROR,
        message=message,
        primary=SourceSpan(path, line),
        repair=repair,
        subject=subject,
    )


def _parse_metadata(
    raw: str,
    path: str,
    line: int,
    subject: NodeId,
) -> tuple[dict[str, str], tuple[Diagnostic, ...]]:
    values: dict[str, str] = {}
    diagnostics: list[Diagnostic] = []
    for token in raw.split():
        if token.count("=") != 1:
            diagnostics.append(
                _diagnostic(
                    "NDF-ASL-002",
                    f"invalid NDF metadata token: {token}",
                    path,
                    line,
                    "Write metadata as one key=value token per field.",
                    subject,
                )
            )
            continue
        name, value = token.split("=", 1)
        if name in values:
            diagnostics.append(
                _diagnostic(
                    "NDF-ASL-002",
                    f"duplicate NDF metadata field: {name}",
                    path,
                    line,
                    "Keep exactly one value for each metadata field.",
                    subject,
                )
            )
        values[name] = value

    required = {"kind", "level", "layer", "status"}
    for name in sorted(required - set(values)):
        diagnostics.append(
            _diagnostic(
                "NDF-ASL-002",
                f"missing NDF metadata field: {name}",
                path,
                line,
                "Add all required PTO NDF metadata fields.",
                subject,
            )
        )
    for name in sorted(set(values) - required):
        diagnostics.append(
            _diagnostic(
                "NDF-ASL-002",
                f"unknown NDF metadata field: {name}",
                path,
                line,
                "Remove the unsupported metadata field.",
                subject,
            )
        )

    kind = values.get("kind")
    if kind not in _KINDS:
        if kind is not None:
            diagnostics.append(
                _diagnostic(
                    "NDF-ASL-002",
                    f"unknown NDF kind: {kind}",
                    path,
                    line,
                    "Use intent, contract, mechanism, or executable.",
                    subject,
                )
            )
    elif values.get("level") != _KINDS[kind][0]:
        diagnostics.append(
            _diagnostic(
                "NDF-ASL-002",
                f"kind {kind} requires level {_KINDS[kind][0]}",
                path,
                line,
                "Use the level assigned to the PTO kind.",
                subject,
            )
        )
    if "layer" in values and values["layer"] not in _LAYERS:
        diagnostics.append(
            _diagnostic(
                "NDF-ASL-002",
                f"unknown NDF layer: {values['layer']}",
                path,
                line,
                "Use a registered PTO layer.",
                subject,
            )
        )
    if "status" in values and values["status"] not in _STATUSES:
        diagnostics.append(
            _diagnostic(
                "NDF-ASL-002",
                f"unknown NDF status: {values['status']}",
                path,
                line,
                "Use open or accepted.",
                subject,
            )
        )
    return values, tuple(diagnostics)


def _instruction_node(
    line: str,
    path: str,
    line_number: int,
    project: ProjectManifest,
) -> tuple[Node | None, Diagnostic | None]:
    try:
        metadata = json.loads(line[len(_INSTRUCTION_PREFIX) :])
        surface = metadata["surface"]
        mnemonic = metadata["mnemonic"]
        local_id = instruction_clause_id(surface, mnemonic)
    except (json.JSONDecodeError, KeyError, TypeError, ValueError) as error:
        return (
            None,
            _diagnostic(
                "NDF-ASL-004",
                f"invalid PTO instruction metadata: {error}",
                path,
                line_number,
                "Provide JSON string fields surface and mnemonic.",
            ),
        )
    return (
        Node(
            id=NodeId(project.project, local_id),
            kind=NodeKind.DEFINITION,
            title=f"{surface} {mnemonic} instruction",
            source=SourceSpan(path, line_number),
            domain=str(surface).lower(),
            status=LifecycleStatus.ACTIVE,
            owner="pto-spec",
            attributes=(("surface", str(surface)), ("mnemonic", str(mnemonic))),
        ),
        None,
    )


def parse_pto_asl(path: Path, project: ProjectManifest) -> ParseResult:
    return parse_pto_asl_text(path.read_text(), path, project)


def parse_pto_asl_text(
    text: str,
    source: Path,
    project: ProjectManifest,
) -> ParseResult:
    path = _relative_path(source, project)
    graph = Graph()
    edges: list[Edge] = []
    diagnostics: list[Diagnostic] = []
    active_id: str | None = None
    active_line = 0
    metadata_line = 0
    metadata_raw: str | None = None
    body_lines: list[str] = []
    region_invalid = False

    for line_number, line in enumerate(text.splitlines(), start=1):
        if active_id is None and line.startswith(_INSTRUCTION_PREFIX):
            node, diagnostic = _instruction_node(line, path, line_number, project)
            if node:
                graph = graph.add_node(node)
            if diagnostic:
                diagnostics.append(diagnostic)
            continue

        begin = _REGION_BEGIN.fullmatch(line)
        end = _REGION_END.fullmatch(line)
        if begin:
            if active_id is not None:
                diagnostics.append(
                    _diagnostic(
                        "NDF-ASL-001",
                        f"nested NDF region: {begin.group(1)}",
                        path,
                        line_number,
                        "Close the active region before starting another.",
                        NodeId(project.project, active_id),
                    )
                )
                region_invalid = True
                continue
            active_id = begin.group(1)
            active_line = line_number
            metadata_line = 0
            metadata_raw = None
            body_lines = []
            region_invalid = False
            continue
        if end:
            if active_id is None:
                diagnostics.append(
                    _diagnostic(
                        "NDF-ASL-001",
                        f"unmatched NDF end: {end.group(1)}",
                        path,
                        line_number,
                        "Remove the end marker or add its begin marker.",
                    )
                )
                continue
            subject = NodeId(project.project, active_id)
            if end.group(1) != active_id:
                diagnostics.append(
                    _diagnostic(
                        "NDF-ASL-001",
                        f"mismatched NDF end {end.group(1)} for {active_id}",
                        path,
                        line_number,
                        "Use the active clause ID in the end marker.",
                        subject,
                    )
                )
                region_invalid = True
            if metadata_raw is None:
                diagnostics.append(
                    _diagnostic(
                        "NDF-ASL-002",
                        f"NDF clause {active_id} has no metadata",
                        path,
                        active_line,
                        "Add the required PTO NDF metadata line.",
                        subject,
                    )
                )
                values = {}
                region_invalid = True
            else:
                values, metadata_diagnostics = _parse_metadata(
                    metadata_raw,
                    path,
                    metadata_line,
                    subject,
                )
                diagnostics.extend(metadata_diagnostics)
                region_invalid = region_invalid or bool(metadata_diagnostics)
            body = "\n".join(body_lines).strip()
            if not body:
                diagnostics.append(
                    _diagnostic(
                        "NDF-ASL-003",
                        f"NDF clause {active_id} has an empty body",
                        path,
                        active_line,
                        "Add one or more ASL comment body lines.",
                        subject,
                    )
                )
                region_invalid = True
            if not region_invalid:
                kind = values["kind"]
                node = Node(
                    id=subject,
                    kind=_KINDS[kind][1],
                    title=active_id.replace("PTO-", "").replace("-", " ").title(),
                    source=SourceSpan(path, active_line),
                    refinement=values["level"],
                    domain=values["layer"],
                    status=_STATUSES[values["status"]],
                    owner="pto-spec",
                    body=body,
                    attributes=(("pto_kind", kind),),
                )
                graph = graph.add_node(node)
                for target in _REFERENCE.findall(body):
                    edges.append(
                        Edge(
                            source=node.id,
                            target=NodeId(project.project, target),
                            type="references",
                            source_span=SourceSpan(path, active_line),
                        )
                    )
            active_id = None
            continue

        if active_id is None:
            if line.startswith("// NDF-BEGIN:"):
                diagnostics.append(
                    _diagnostic(
                        "NDF-ASL-001",
                        "invalid PTO NDF clause ID",
                        path,
                        line_number,
                        "Use a PTO-prefixed uppercase clause ID.",
                    )
                )
            continue
        if line.startswith(_METADATA_PREFIX):
            if metadata_raw is not None:
                diagnostics.append(
                    _diagnostic(
                        "NDF-ASL-002",
                        "duplicate NDF metadata line",
                        path,
                        line_number,
                        "Keep exactly one metadata line per region.",
                        NodeId(project.project, active_id),
                    )
                )
                region_invalid = True
            else:
                metadata_raw = line[len(_METADATA_PREFIX) :]
                metadata_line = line_number
        elif line.startswith("//"):
            body_lines.append(line[2:].removeprefix(" "))
        else:
            diagnostics.append(
                _diagnostic(
                    "NDF-ASL-003",
                    "NDF body line must be an ASL comment",
                    path,
                    line_number,
                    "Prefix normative body lines with //.",
                    NodeId(project.project, active_id),
                )
            )
            region_invalid = True

    if active_id is not None:
        diagnostics.append(
            _diagnostic(
                "NDF-ASL-001",
                f"unterminated NDF clause {active_id}",
                path,
                active_line,
                "Add the matching NDF-END marker.",
                NodeId(project.project, active_id),
            )
        )
    return ParseResult(
        nodes=graph.nodes,
        edges=tuple(edges),
        diagnostics=graph.diagnostics + tuple(diagnostics),
    )
