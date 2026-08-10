from dataclasses import dataclass

from .index import NdfIndex


@dataclass(frozen=True)
class CoveragePolicy:
    refinement: str = "L1"


@dataclass(frozen=True)
class CoverageReport:
    unverified: tuple[str, ...]
    open_items: tuple[str, ...]
    tbd: tuple[str, ...]

    def to_dict(self) -> dict[str, list[str]]:
        return {
            "unverified": list(self.unverified),
            "open_items": list(self.open_items),
            "tbd": list(self.tbd),
        }


def coverage(index: NdfIndex, policy: CoveragePolicy) -> CoverageReport:
    unverified: list[str] = []
    open_items: list[str] = []
    tbd: list[str] = []
    for node in index.all_nodes():
        uri = str(node.id)
        if (
            node.kind == "requirement"
            and node.status == "active"
            and node.modality == "must"
            and node.refinement == policy.refinement
            and not index.in_edges(uri, "verifies")
        ):
            unverified.append(uri)
        if node.kind in {"question", "option"} and node.status == "open":
            open_items.append(uri)
        if node.modality == "tbd":
            tbd.append(uri)
    return CoverageReport(
        tuple(sorted(unverified)),
        tuple(sorted(open_items)),
        tuple(sorted(tbd)),
    )
