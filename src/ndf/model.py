from dataclasses import dataclass
from enum import StrEnum
from typing import Any

from .identity import NodeId


class NodeKind(StrEnum):
    DOCUMENT = "document"
    REQUIREMENT = "requirement"
    DEFINITION = "definition"
    ARCHITECTURE = "architecture"
    CONSTRAINT = "constraint"
    OPTION = "option"
    VERIFICATION = "verification"
    DECISION = "decision"
    QUESTION = "question"
    MODEL = "model"
    SOURCE = "source"
    TEST = "test"
    EVIDENCE = "evidence"
    INFORMATION = "information"


class Modality(StrEnum):
    MUST = "must"
    SHOULD = "should"
    MAY = "may"
    TBD = "tbd"


class LifecycleStatus(StrEnum):
    DRAFT = "draft"
    ACTIVE = "active"
    DEPRECATED = "deprecated"
    SUPERSEDED = "superseded"
    RETIRED = "retired"
    OPEN = "open"
    RESOLVED = "resolved"


class DiagnosticSeverity(StrEnum):
    INFORMATION = "information"
    WARNING = "warning"
    ERROR = "error"


@dataclass(frozen=True, order=True)
class SourceSpan:
    path: str
    line: int
    column: int = 1
    end_line: int | None = None
    end_column: int | None = None

    def __post_init__(self) -> None:
        if self.line < 1:
            raise ValueError("line must be positive")
        if self.column < 1:
            raise ValueError("column must be positive")
        if self.end_line is not None and self.end_line < self.line:
            raise ValueError("end line must not precede start line")
        if self.end_column is not None and self.end_column < 1:
            raise ValueError("end column must be positive")

    def to_dict(self) -> dict[str, Any]:
        return {
            "path": self.path,
            "line": self.line,
            "column": self.column,
            "end_line": self.end_line,
            "end_column": self.end_column,
        }


@dataclass(frozen=True)
class Node:
    id: NodeId
    kind: NodeKind
    title: str
    source: SourceSpan
    modality: Modality | None = None
    refinement: str | None = None
    domain: str | None = None
    status: LifecycleStatus = LifecycleStatus.DRAFT
    owner: str | None = None
    body: str = ""
    attributes: tuple[tuple[str, str], ...] = ()

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": str(self.id),
            "kind": self.kind.value,
            "title": self.title,
            "source": self.source.to_dict(),
            "modality": self.modality.value if self.modality else None,
            "refinement": self.refinement,
            "domain": self.domain,
            "status": self.status.value,
            "owner": self.owner,
            "body": self.body,
            "attributes": dict(self.attributes),
        }


@dataclass(frozen=True)
class Edge:
    source: NodeId
    target: NodeId
    type: str
    source_span: SourceSpan | None = None
    attributes: tuple[tuple[str, str], ...] = ()

    def __post_init__(self) -> None:
        if not self.type.strip():
            raise ValueError("edge type must not be empty")

    def to_dict(self) -> dict[str, Any]:
        return {
            "source": str(self.source),
            "target": str(self.target),
            "type": self.type,
            "source_span": self.source_span.to_dict() if self.source_span else None,
            "attributes": dict(self.attributes),
        }


@dataclass(frozen=True)
class Diagnostic:
    code: str
    severity: DiagnosticSeverity
    message: str
    primary: SourceSpan
    related: tuple[SourceSpan, ...] = ()
    repair: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "code": self.code,
            "severity": self.severity.value,
            "message": self.message,
            "primary": self.primary.to_dict(),
            "related": [span.to_dict() for span in self.related],
            "repair": self.repair,
        }


@dataclass(frozen=True)
class Graph:
    nodes: tuple[Node, ...] = ()
    edges: tuple[Edge, ...] = ()
    diagnostics: tuple[Diagnostic, ...] = ()

    def add_node(self, node: Node) -> "Graph":
        existing = next((item for item in self.nodes if item.id == node.id), None)
        if existing is None:
            return Graph(self.nodes + (node,), self.edges, self.diagnostics)
        diagnostic = Diagnostic(
            code="NDF-ID-001",
            severity=DiagnosticSeverity.ERROR,
            message=f"duplicate canonical NDF ID: {node.id}",
            primary=node.source,
            related=(existing.source,),
            repair="Assign a unique explicit ID or remove the duplicate clause.",
        )
        return Graph(self.nodes, self.edges, self.diagnostics + (diagnostic,))

    def sorted_nodes(self) -> tuple[Node, ...]:
        return tuple(sorted(self.nodes, key=lambda node: str(node.id)))

    def sorted_edges(self) -> tuple[Edge, ...]:
        return tuple(
            sorted(
                self.edges,
                key=lambda edge: (str(edge.source), edge.type, str(edge.target)),
            )
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": "0.1",
            "nodes": [node.to_dict() for node in self.sorted_nodes()],
            "edges": [edge.to_dict() for edge in self.sorted_edges()],
            "diagnostics": [
                diagnostic.to_dict()
                for diagnostic in sorted(
                    self.diagnostics,
                    key=lambda item: (
                        item.primary.path,
                        item.primary.line,
                        item.code,
                    ),
                )
            ],
        }


@dataclass(frozen=True)
class ParseResult:
    nodes: tuple[Node, ...] = ()
    edges: tuple[Edge, ...] = ()
    diagnostics: tuple[Diagnostic, ...] = ()
