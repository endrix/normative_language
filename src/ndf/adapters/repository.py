import hashlib
import re
import subprocess
from pathlib import Path
from collections.abc import Sequence

from ndf.identity import NodeId, parse_uri
from ndf.manifest import ProjectManifest
from ndf.model import Edge, LifecycleStatus, Node, NodeKind, ParseResult, SourceSpan


_LINKS = re.compile(r"^\s*(?:#|//)\s*ndf:\s*(.*?)\s*$")
_EDGE_TYPES = {"implements", "verifies"}


def _revision(root: Path) -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        text=True,
        capture_output=True,
        check=False,
    )
    value = result.stdout.strip()
    return value if len(value) == 40 else "0" * 40


def _kind(path: Path) -> NodeKind:
    if "tests" in path.parts or path.name.startswith("test_"):
        return NodeKind.TEST
    return NodeKind.SOURCE


def _derived_id(project: str, path: str, kind: NodeKind) -> str:
    digest = hashlib.sha256(f"{project}\0{kind.value}\0{path}".encode()).hexdigest()
    prefix = "TEST" if kind is NodeKind.TEST else "SRC"
    return f"{prefix}-{digest[:16].upper()}"


def _edges(
    node: Node,
    content: str,
    project: ProjectManifest,
) -> tuple[Edge, ...]:
    edges: list[Edge] = []
    for line_number, line in enumerate(content.splitlines(), start=1):
        match = _LINKS.match(line)
        if not match:
            continue
        for token in match.group(1).split():
            if "=" not in token:
                continue
            edge_type, targets = token.split("=", 1)
            if edge_type not in _EDGE_TYPES:
                continue
            for target in targets.split(","):
                edges.append(
                    Edge(
                        source=node.id,
                        target=parse_uri(target, current_project=project.project),
                        type=edge_type,
                        source_span=SourceSpan(node.source.path, line_number),
                    )
                )
    return tuple(edges)


def index_repository_paths(
    root: Path,
    globs: Sequence[str],
    project: ProjectManifest,
) -> ParseResult:
    paths = sorted(
        {
            path
            for pattern in globs
            for path in root.glob(pattern)
            if path.is_file()
        },
        key=lambda path: path.relative_to(root).as_posix(),
    )
    revision = _revision(root)
    nodes: list[Node] = []
    edges: list[Edge] = []
    for path in paths:
        relative = path.relative_to(root).as_posix()
        content = path.read_text()
        kind = _kind(Path(relative))
        node = Node(
            id=NodeId(project.project, _derived_id(project.project, relative, kind)),
            kind=kind,
            title=relative,
            source=SourceSpan(relative, 1),
            status=LifecycleStatus.ACTIVE,
            owner=project.project,
            attributes=tuple(
                sorted(
                    {
                        "path": relative,
                        "revision": revision,
                        "content_hash": hashlib.sha256(content.encode()).hexdigest(),
                    }.items()
                )
            ),
        )
        nodes.append(node)
        edges.extend(_edges(node, content, project))
    return ParseResult(tuple(nodes), tuple(edges), ())
