from dataclasses import dataclass

from .index import IndexedEdge, NdfIndex, NodeRecord


_QUERY_FIELDS = {
    "kind",
    "modality",
    "refinement",
    "domain",
    "status",
    "owner",
}


def query_nodes(index: NdfIndex, expression: str) -> tuple[NodeRecord, ...]:
    filters: list[tuple[str, str]] = []
    for clause in expression.split(" and "):
        if "=" not in clause:
            raise ValueError(f"invalid query clause: {clause}")
        field, value = (part.strip() for part in clause.split("=", 1))
        if field not in _QUERY_FIELDS:
            raise ValueError(f"unsupported query field: {field}")
        filters.append((field, value))
    return tuple(
        node
        for node in index.all_nodes()
        if all(getattr(node, field) == value for field, value in filters)
    )


@dataclass(frozen=True)
class TraceEdge:
    source: str
    type: str
    target: str

    @classmethod
    def from_indexed(cls, edge: IndexedEdge) -> "TraceEdge":
        return cls(str(edge.source), edge.type, str(edge.target))

    def to_dict(self) -> dict[str, str]:
        return {"source": self.source, "type": self.type, "target": self.target}


@dataclass(frozen=True)
class TraceTree:
    root: str
    nodes: tuple[str, ...]
    edges: tuple[TraceEdge, ...]
    cycles: tuple[TraceEdge, ...]

    def to_dict(self) -> dict:
        return {
            "root": self.root,
            "nodes": list(self.nodes),
            "edges": [edge.to_dict() for edge in self.edges],
            "cycles": [edge.to_dict() for edge in self.cycles],
        }


def trace(
    index: NdfIndex,
    uri: str,
    edge_types: frozenset[str],
) -> TraceTree:
    root = index.resolve_uri(uri)
    visited: set[str] = set()
    active: set[str] = set()
    nodes: list[str] = []
    edges: list[TraceEdge] = []
    cycles: list[TraceEdge] = []

    def visit(current: str) -> None:
        if current not in visited:
            visited.add(current)
            nodes.append(current)
        active.add(current)
        for indexed_edge in index.out_edges(current):
            if indexed_edge.type not in edge_types:
                continue
            edge = TraceEdge.from_indexed(indexed_edge)
            edges.append(edge)
            if edge.target in active:
                cycles.append(edge)
                continue
            if edge.target not in visited:
                visit(edge.target)
        active.remove(current)

    visit(root)
    return TraceTree(root, tuple(nodes), tuple(edges), tuple(cycles))
