import hashlib
import json
import os
import sqlite3
import tempfile
from dataclasses import dataclass
from importlib import resources
from pathlib import Path

from .baseline import diagnostic_fingerprint
from .identity import NodeId, parse_uri
from .model import Edge, Graph, SourceSpan


@dataclass(frozen=True)
class BuildProvenance:
    project_commits: tuple[tuple[str, str], ...]
    format_version: str
    ir_version: str
    tool_version: str
    normative_roots: tuple[Path, ...] = ()


@dataclass(frozen=True)
class IndexBuild:
    output: Path
    semantic_hash: str
    node_count: int
    edge_count: int
    diagnostics: tuple


@dataclass(frozen=True)
class IndexedNode:
    id: NodeId
    kind: str
    title: str
    source: SourceSpan
    modality: str | None
    refinement: str | None
    domain: str | None
    status: str
    owner: str | None
    body: str


@dataclass(frozen=True)
class IndexedEdge:
    source: NodeId
    target: NodeId
    type: str
    source_span: SourceSpan | None


class NdfIndex:
    def __init__(self, path: Path) -> None:
        self.path = path
        self._connection = sqlite3.connect(path)
        self._connection.row_factory = sqlite3.Row
        self._connection.execute("PRAGMA foreign_keys = ON")

    def __enter__(self) -> "NdfIndex":
        return self

    def __exit__(self, *_: object) -> None:
        self.close()

    def close(self) -> None:
        self._connection.close()

    def foreign_keys_enabled(self) -> bool:
        row = self._connection.execute("PRAGMA foreign_keys").fetchone()
        return bool(row[0])

    def get_node(self, uri: str) -> IndexedNode | None:
        row = self._connection.execute(
            "SELECT * FROM nodes WHERE uri = ?",
            (str(parse_uri(uri)),),
        ).fetchone()
        if row is None:
            return None
        return IndexedNode(
            id=parse_uri(row["uri"]),
            kind=row["kind"],
            title=row["title"],
            source=SourceSpan(
                row["source_path"],
                row["line"],
                row["column_number"],
            ),
            modality=row["modality"],
            refinement=row["refinement"],
            domain=row["domain"],
            status=row["status"],
            owner=row["owner"],
            body=row["body"],
        )

    def out_edges(
        self,
        uri: str,
        edge_type: str | None = None,
    ) -> tuple[IndexedEdge, ...]:
        return self._edges("source_uri", uri, edge_type)

    def in_edges(
        self,
        uri: str,
        edge_type: str | None = None,
    ) -> tuple[IndexedEdge, ...]:
        return self._edges("target_uri", uri, edge_type)

    def _edges(
        self,
        direction: str,
        uri: str,
        edge_type: str | None,
    ) -> tuple[IndexedEdge, ...]:
        parameters: tuple[str, ...]
        if edge_type is None:
            query = (
                f"SELECT * FROM edges WHERE {direction} = ? "
                "ORDER BY source_uri, edge_type, target_uri, edge_id"
            )
            parameters = (str(parse_uri(uri)),)
        else:
            query = (
                f"SELECT * FROM edges WHERE {direction} = ? AND edge_type = ? "
                "ORDER BY source_uri, edge_type, target_uri, edge_id"
            )
            parameters = (str(parse_uri(uri)), edge_type)
        return tuple(self._indexed_edge(row) for row in self._connection.execute(query, parameters))

    @staticmethod
    def _indexed_edge(row: sqlite3.Row) -> IndexedEdge:
        span = (
            SourceSpan(row["source_path"], row["line"])
            if row["source_path"] is not None
            else None
        )
        return IndexedEdge(
            source=parse_uri(row["source_uri"]),
            target=parse_uri(row["target_uri"]),
            type=row["edge_type"],
            source_span=span,
        )

    def provenance(self) -> dict[str, str]:
        row = self._connection.execute(
            "SELECT format_version, ir_version, semantic_hash, tool_version "
            "FROM builds WHERE build_id = 1"
        ).fetchone()
        if row is None:
            raise ValueError("index has no build provenance")
        return dict(row)


def open_index(path: Path) -> NdfIndex:
    return NdfIndex(path)


def _semantic_hash(graph: Graph, provenance: BuildProvenance) -> str:
    payload = {
        "format_version": provenance.format_version,
        "graph": graph.to_dict(),
        "ir_version": provenance.ir_version,
        "project_commits": sorted(provenance.project_commits),
        "tool_version": provenance.tool_version,
    }
    encoded = json.dumps(
        payload,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode()
    return hashlib.sha256(encoded).hexdigest()


def _validate_build(
    graph: Graph,
    output: Path,
    provenance: BuildProvenance,
) -> None:
    revisions = dict(provenance.project_commits)
    missing = sorted(
        {node.id.project_id for node in graph.nodes if node.id.project_id not in revisions}
    )
    if missing:
        raise ValueError(f"missing project revision for: {missing[0]}")
    resolved_output = output.resolve()
    for root in provenance.normative_roots:
        if resolved_output.is_relative_to(root.resolve()):
            raise ValueError(f"refusing to write SQLite index inside normative root: {root}")


def _source_paths(graph: Graph) -> tuple[str, ...]:
    paths = {node.source.path for node in graph.nodes}
    for edge in graph.edges:
        if edge.source_span:
            paths.add(edge.source_span.path)
    for diagnostic in graph.diagnostics:
        paths.add(diagnostic.primary.path)
        paths.update(span.path for span in diagnostic.related)
    return tuple(sorted(paths))


def _insert_graph(
    connection: sqlite3.Connection,
    graph: Graph,
    provenance: BuildProvenance,
    semantic_hash: str,
) -> None:
    connection.executemany(
        "INSERT INTO projects(project_id, revision) VALUES (?, ?)",
        sorted(provenance.project_commits),
    )
    connection.executemany(
        "INSERT INTO sources(path) VALUES (?)",
        ((path,) for path in _source_paths(graph)),
    )

    for node in graph.sorted_nodes():
        connection.execute(
            """INSERT INTO nodes(
                uri, project_id, local_id, kind, title, source_path, line,
                column_number, modality, refinement, domain, status, owner, body
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)""",
            (
                str(node.id),
                node.id.project_id,
                node.id.local_id,
                node.kind.value,
                node.title,
                node.source.path,
                node.source.line,
                node.source.column,
                node.modality.value if node.modality else None,
                node.refinement,
                node.domain,
                node.status.value,
                node.owner,
                node.body,
            ),
        )
        connection.executemany(
            "INSERT INTO node_attrs(node_uri, key, value) VALUES (?, ?, ?)",
            ((str(node.id), key, value) for key, value in sorted(node.attributes)),
        )

    for edge_id, edge in enumerate(graph.sorted_edges(), start=1):
        connection.execute(
            """INSERT INTO edges(
                edge_id, source_uri, edge_type, target_uri, source_path, line,
                attributes_json
            ) VALUES (?, ?, ?, ?, ?, ?, ?)""",
            (
                edge_id,
                str(edge.source),
                edge.type,
                str(edge.target),
                edge.source_span.path if edge.source_span else None,
                edge.source_span.line if edge.source_span else None,
                json.dumps(dict(edge.attributes), sort_keys=True, separators=(",", ":")),
            ),
        )

    ordered_diagnostics = sorted(
        graph.diagnostics,
        key=lambda item: (
            item.primary.path,
            item.primary.line,
            item.code,
            str(item.subject) if item.subject else "",
        ),
    )
    for diagnostic_id, diagnostic in enumerate(ordered_diagnostics, start=1):
        connection.execute(
            """INSERT INTO diagnostics(
                diagnostic_id, fingerprint, code, severity, message, subject_uri,
                source_path, line, repair
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)""",
            (
                diagnostic_id,
                diagnostic_fingerprint(diagnostic),
                diagnostic.code,
                diagnostic.severity.value,
                diagnostic.message,
                str(diagnostic.subject) if diagnostic.subject else None,
                diagnostic.primary.path,
                diagnostic.primary.line,
                diagnostic.repair,
            ),
        )

    connection.execute(
        """INSERT INTO builds(
            build_id, schema_version, semantic_hash, format_version, ir_version,
            tool_version
        ) VALUES (1, '1', ?, ?, ?, ?)""",
        (
            semantic_hash,
            provenance.format_version,
            provenance.ir_version,
            provenance.tool_version,
        ),
    )


def build_index(
    graph: Graph,
    output: Path,
    provenance: BuildProvenance,
) -> IndexBuild:
    _validate_build(graph, output, provenance)
    output.parent.mkdir(parents=True, exist_ok=True)
    semantic_hash = _semantic_hash(graph, provenance)
    schema = resources.files("ndf").joinpath("sql/schema-v1.sql").read_text()
    temporary_file = tempfile.NamedTemporaryFile(
        prefix=f".{output.name}.",
        suffix=".tmp",
        dir=output.parent,
        delete=False,
    )
    temporary_path = Path(temporary_file.name)
    temporary_file.close()

    try:
        with sqlite3.connect(temporary_path) as connection:
            connection.execute("PRAGMA foreign_keys = ON")
            connection.executescript(schema)
            _insert_graph(connection, graph, provenance, semantic_hash)
            violations = connection.execute("PRAGMA foreign_key_check").fetchall()
            if violations:
                raise ValueError(f"SQLite foreign key violations: {violations}")
        os.replace(temporary_path, output)
    except Exception:
        temporary_path.unlink(missing_ok=True)
        raise

    return IndexBuild(
        output=output,
        semantic_hash=semantic_hash,
        node_count=len(graph.nodes),
        edge_count=len(graph.edges),
        diagnostics=graph.diagnostics,
    )
