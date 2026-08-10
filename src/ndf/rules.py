import re
from dataclasses import dataclass, field

from .model import (
    Diagnostic,
    DiagnosticSeverity,
    Graph,
    LifecycleStatus,
    Modality,
    Node,
    NodeKind,
    SourceSpan,
)


_MUST = re.compile(r"\bMUST\b")


@dataclass(frozen=True)
class ValidationPolicy:
    mandatory_edge_types: frozenset[str] = field(
        default_factory=lambda: frozenset(
            {"refines", "depends-on", "verifies", "implements"}
        )
    )
    ownership_kinds: frozenset[NodeKind] = field(
        default_factory=lambda: frozenset(
            {
                NodeKind.REQUIREMENT,
                NodeKind.CONSTRAINT,
                NodeKind.VERIFICATION,
                NodeKind.DECISION,
                NodeKind.QUESTION,
            }
        )
    )


def _node_diagnostic(
    code: str,
    message: str,
    node: Node,
    repair: str,
) -> Diagnostic:
    return Diagnostic(
        code=code,
        severity=DiagnosticSeverity.ERROR,
        message=message,
        primary=node.source,
        repair=repair,
        subject=node.id,
    )


def validate_graph(
    graph: Graph,
    policy: ValidationPolicy,
) -> tuple[Diagnostic, ...]:
    diagnostics = list(graph.diagnostics)
    nodes_by_id = {node.id: node for node in graph.nodes}

    for edge in graph.edges:
        if edge.type not in policy.mandatory_edge_types:
            continue
        missing = edge.source not in nodes_by_id or edge.target not in nodes_by_id
        if not missing:
            continue
        source = nodes_by_id.get(edge.source)
        primary = edge.source_span or (
            source.source if source else SourceSpan("<graph>", 1)
        )
        diagnostics.append(
            Diagnostic(
                code="NDF-EDGE-001",
                severity=DiagnosticSeverity.ERROR,
                message=f"mandatory edge {edge.type} has a dangling endpoint",
                primary=primary,
                repair="Resolve both endpoints or remove the mandatory edge.",
                subject=edge.source,
            )
        )

    for node in graph.nodes:
        if node.modality is Modality.MUST and not _MUST.search(node.body):
            diagnostics.append(
                _node_diagnostic(
                    "NDF-MOD-001",
                    f"must requirement {node.id} does not contain MUST",
                    node,
                    "Add normative MUST language or change the modality.",
                )
            )
        if node.kind is NodeKind.INFORMATION and _MUST.search(node.body):
            diagnostics.append(
                _node_diagnostic(
                    "NDF-MOD-002",
                    f"information node {node.id} contains MUST",
                    node,
                    "Move the statement to a normative node or remove MUST.",
                )
            )

        previous_status = dict(node.attributes).get("previous_status")
        if previous_status == LifecycleStatus.ACTIVE.value and node.status is LifecycleStatus.DRAFT:
            diagnostics.append(
                _node_diagnostic(
                    "NDF-LIFE-001",
                    f"node {node.id} cannot move from active to draft",
                    node,
                    "Deprecate or supersede the active node instead.",
                )
            )

        if node.kind in policy.ownership_kinds and not node.owner:
            diagnostics.append(
                _node_diagnostic(
                    "NDF-OWN-001",
                    f"explicit normative node {node.id} has no owner",
                    node,
                    "Assign an accountable owner.",
                )
            )

    return tuple(
        sorted(
            diagnostics,
            key=lambda item: (
                item.primary.path,
                item.primary.line,
                item.code,
                str(item.subject) if item.subject else "",
            ),
        )
    )
