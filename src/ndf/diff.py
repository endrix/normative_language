from dataclasses import dataclass

from .index import NdfIndex, NodeRecord


EdgeTuple = tuple[str, str, str]


@dataclass(frozen=True)
class SemanticDiff:
    added: tuple[str, ...]
    removed: tuple[str, ...]
    modified: tuple[str, ...]
    moved: tuple[str, ...]
    superseded: tuple[str, ...]
    edge_only: tuple[str, ...]
    edges_added: tuple[EdgeTuple, ...]
    edges_removed: tuple[EdgeTuple, ...]

    def to_dict(self) -> dict:
        return {
            "added": list(self.added),
            "removed": list(self.removed),
            "modified": list(self.modified),
            "moved": list(self.moved),
            "superseded": list(self.superseded),
            "edge_only": list(self.edge_only),
            "edges_added": [list(edge) for edge in self.edges_added],
            "edges_removed": [list(edge) for edge in self.edges_removed],
        }


def _semantic_record(node: NodeRecord) -> tuple:
    return (
        node.kind,
        node.title,
        node.modality,
        node.refinement,
        node.domain,
        node.status,
        node.owner,
        node.body,
    )


def _edge_set(index: NdfIndex) -> set[EdgeTuple]:
    return {
        (str(edge.source), edge.type, str(edge.target))
        for edge in index.all_edges()
    }


def semantic_diff(before: NdfIndex, after: NdfIndex) -> SemanticDiff:
    before_nodes = {str(node.id): node for node in before.all_nodes()}
    after_nodes = {str(node.id): node for node in after.all_nodes()}
    before_uris = set(before_nodes)
    after_uris = set(after_nodes)
    added = after_uris - before_uris
    removed = before_uris - after_uris
    modified: set[str] = set()
    moved: set[str] = set()
    superseded: set[str] = set()

    for uri in before_uris & after_uris:
        old = before_nodes[uri]
        new = after_nodes[uri]
        if old.status != "superseded" and new.status == "superseded":
            superseded.add(uri)
            continue
        if _semantic_record(old) != _semantic_record(new):
            modified.add(uri)
        elif old.source != new.source:
            moved.add(uri)

    old_edges = _edge_set(before)
    new_edges = _edge_set(after)
    edges_added = new_edges - old_edges
    edges_removed = old_edges - new_edges
    edge_nodes = {
        endpoint
        for source, _, target in edges_added | edges_removed
        for endpoint in (source, target)
    }
    classified = added | removed | modified | moved | superseded
    edge_only = edge_nodes - classified
    return SemanticDiff(
        tuple(sorted(added)),
        tuple(sorted(removed)),
        tuple(sorted(modified)),
        tuple(sorted(moved)),
        tuple(sorted(superseded)),
        tuple(sorted(edge_only)),
        tuple(sorted(edges_added)),
        tuple(sorted(edges_removed)),
    )
