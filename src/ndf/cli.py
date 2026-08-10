import argparse
import json
import subprocess
from collections.abc import Sequence
from pathlib import Path

from .diff import semantic_diff
from .export import ExportFormat, export_graph
from .fix import FixMode, safe_fix
from .id_allocator import allocate_id
from .index import BuildProvenance, build_index, open_index
from .manifest import ProjectManifest, load_manifest
from .model import DiagnosticSeverity, Graph
from .parsers.markdown import parse_markdown
from .query import trace
from .report import CoveragePolicy, coverage
from .rules import ValidationPolicy, validate_graph
from .version import (
    CLI_STABILITY,
    FORMAT_VERSION,
    IR_VERSION,
    PLUGIN_API,
    TOOL_VERSION,
)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="ndf")
    parser.add_argument("--machine-version", action="store_true")
    commands = parser.add_subparsers(dest="command")

    build = commands.add_parser("build")
    build.add_argument("--root", type=Path, required=True)
    build.add_argument("--output", type=Path, required=True)
    build.add_argument("--format", choices=("json",), default="json")

    check = commands.add_parser("check")
    check.add_argument("--root", type=Path, required=True)
    check.add_argument("--format", choices=("json",), default="json")

    show = commands.add_parser("show")
    show.add_argument("uri")
    show.add_argument("--index", type=Path, default=Path("graph.sqlite"))
    show.add_argument("--format", choices=("json",), default="json")

    trace_command = commands.add_parser("trace")
    trace_command.add_argument("uri")
    trace_command.add_argument("--edge", action="append", default=[])
    trace_command.add_argument("--index", type=Path, default=Path("graph.sqlite"))
    trace_command.add_argument("--format", choices=("json",), default="json")

    report = commands.add_parser("report")
    report.add_argument("report_name", choices=("coverage",))
    report.add_argument("--index", type=Path, default=Path("graph.sqlite"))
    report.add_argument("--format", choices=("json",), default="json")

    diff = commands.add_parser("diff")
    diff.add_argument("before", type=Path)
    diff.add_argument("after", type=Path)
    diff.add_argument("--format", choices=("json",), default="json")

    export = commands.add_parser("export")
    export.add_argument("--index", type=Path, required=True)
    export.add_argument("--output", type=Path, required=True)
    export.add_argument("--format", choices=("ndf-json",), required=True)

    identity = commands.add_parser("id")
    identity.add_argument("id_command", choices=("allocate",))
    identity.add_argument("prefix")
    identity.add_argument("--index", type=Path, default=Path("graph.sqlite"))
    identity.add_argument("--format", choices=("json",), default="json")

    fix = commands.add_parser("fix")
    fix.add_argument("paths", nargs="*", type=Path)
    fix.add_argument("--generated-only", action="store_true")
    fix.add_argument("--check", action="store_true")
    return parser


def _version_payload() -> dict[str, str]:
    return {
        "cli_stability": CLI_STABILITY,
        "format_version": FORMAT_VERSION,
        "ir_version": IR_VERSION,
        "plugin_api": PLUGIN_API,
        "tool_version": TOOL_VERSION,
    }


def _emit(command: str, ok: bool, data: object, diagnostics: Sequence[dict] = ()) -> int:
    print(
        json.dumps(
            {
                "schema_version": "0.1",
                "command": command,
                "ok": ok,
                "data": data,
                "diagnostics": list(diagnostics),
            },
            sort_keys=True,
        )
    )
    return 0 if ok else 1


def _project_graph(root: Path) -> tuple[ProjectManifest, Graph]:
    manifest = load_manifest(root / "ndf.yaml")
    graph = Graph()
    edges = []
    diagnostics = []
    for pattern in manifest.roots:
        for path in sorted(root.glob(pattern)):
            if not path.is_file():
                continue
            result = parse_markdown(path, manifest)
            for node in result.nodes:
                graph = graph.add_node(node)
            edges.extend(result.edges)
            diagnostics.extend(result.diagnostics)
    parsed = Graph(
        nodes=graph.nodes,
        edges=tuple(edges),
        diagnostics=graph.diagnostics + tuple(diagnostics),
    )
    validated = validate_graph(parsed, ValidationPolicy())
    return manifest, Graph(parsed.nodes, parsed.edges, validated)


def _revision(root: Path) -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        text=True,
        capture_output=True,
        check=False,
    )
    revision = result.stdout.strip()
    return revision if len(revision) == 40 else "0" * 40


def _normative_roots(root: Path, manifest: ProjectManifest) -> tuple[Path, ...]:
    roots = set()
    for pattern in manifest.roots:
        prefix = pattern.split("*", 1)[0].rstrip("/")
        roots.add(root / prefix if prefix else root)
    return tuple(sorted(roots))


def _build_provenance(root: Path, manifest: ProjectManifest) -> BuildProvenance:
    return BuildProvenance(
        project_commits=((manifest.project, _revision(root)),),
        format_version=FORMAT_VERSION,
        ir_version=IR_VERSION,
        tool_version=TOOL_VERSION,
        normative_roots=_normative_roots(root, manifest),
    )


def _diagnostics(graph: Graph) -> list[dict]:
    return [diagnostic.to_dict() for diagnostic in graph.diagnostics]


def _has_errors(graph: Graph) -> bool:
    return any(
        diagnostic.severity is DiagnosticSeverity.ERROR
        for diagnostic in graph.diagnostics
    )


def _run(args: argparse.Namespace) -> int:
    if args.command == "build":
        manifest, graph = _project_graph(args.root)
        build = build_index(
            graph,
            args.output,
            _build_provenance(args.root, manifest),
        )
        return _emit(
            "build",
            not _has_errors(graph),
            {
                "output": str(build.output),
                "semantic_hash": build.semantic_hash,
                "node_count": build.node_count,
                "edge_count": build.edge_count,
            },
            _diagnostics(graph),
        )
    if args.command == "check":
        _, graph = _project_graph(args.root)
        return _emit(
            "check",
            not _has_errors(graph),
            {"node_count": len(graph.nodes), "edge_count": len(graph.edges)},
            _diagnostics(graph),
        )
    if args.command == "show":
        with open_index(args.index) as index:
            uri = index.resolve_uri(args.uri)
            node = index.get_node(uri)
            if node is None:
                raise ValueError(f"unknown NDF ID: {args.uri}")
            return _emit("show", True, node.to_dict())
    if args.command == "trace":
        edge_types = frozenset(args.edge or ["refines"])
        with open_index(args.index) as index:
            return _emit("trace", True, trace(index, args.uri, edge_types).to_dict())
    if args.command == "report":
        with open_index(args.index) as index:
            report = coverage(index, CoveragePolicy())
            return _emit("report coverage", True, report.to_dict())
    if args.command == "diff":
        with open_index(args.before) as before, open_index(args.after) as after:
            return _emit("diff", True, semantic_diff(before, after).to_dict())
    if args.command == "export":
        with open_index(args.index) as index:
            result = export_graph(index, args.output, ExportFormat(args.format))
            return _emit("export", True, result.to_dict())
    if args.command == "id":
        occupied: tuple[str, ...] = ()
        if args.index.exists():
            with open_index(args.index) as index:
                occupied = tuple(node.id.local_id for node in index.all_nodes())
        return _emit(
            "id allocate",
            True,
            {"id": allocate_id(args.prefix, occupied)},
        )
    if args.command == "fix":
        paths = tuple(args.paths) or tuple(sorted(Path.cwd().rglob("*.md")))
        result = safe_fix(
            paths,
            FixMode(generated_only=args.generated_only, check=args.check),
        )
        return _emit(
            "fix",
            not result.diagnostics,
            {"changed": list(result.changed)},
            [diagnostic.to_dict() for diagnostic in result.diagnostics],
        )
    raise ValueError("a command is required")


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    if args.machine_version:
        print(json.dumps(_version_payload(), sort_keys=True))
        return 0
    try:
        return _run(args)
    except Exception as error:
        command = args.command or "unknown"
        return _emit(command, False, {}, ({"message": str(error)},))


def entrypoint() -> None:
    raise SystemExit(main())
