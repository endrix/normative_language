import json
from dataclasses import dataclass
from enum import StrEnum
from pathlib import Path, PurePosixPath

from .index import NdfIndex


class ExportFormat(StrEnum):
    NDF_JSON = "ndf-json"


@dataclass(frozen=True)
class ExportResult:
    output: Path
    semantic_hash: str
    node_count: int
    edge_count: int

    def to_dict(self) -> dict:
        return {
            "output": str(self.output),
            "semantic_hash": self.semantic_hash,
            "node_count": self.node_count,
            "edge_count": self.edge_count,
        }


def _portable_path(value: str) -> str:
    path = Path(value)
    if path.is_absolute():
        return PurePosixPath("<external>", path.name).as_posix()
    return PurePosixPath(value.replace("\\", "/")).as_posix()


def _portable_node(record: dict) -> dict:
    record["source"]["path"] = _portable_path(record["source"]["path"])
    return record


def _portable_edge(record: dict) -> dict:
    if record["source_span"]:
        record["source_span"]["path"] = _portable_path(
            record["source_span"]["path"]
        )
    return record


def export_graph(
    index: NdfIndex,
    output: Path,
    format: ExportFormat,
) -> ExportResult:
    if format is not ExportFormat.NDF_JSON:
        raise ValueError(f"unsupported export format: {format}")
    provenance = index.provenance()
    diagnostics = []
    for record in index.diagnostic_records():
        diagnostics.append(
            {
                "code": record["code"],
                "severity": record["severity"],
                "message": record["message"],
                "primary": {
                    "path": _portable_path(record["source_path"]),
                    "line": record["line"],
                    "column": 1,
                    "end_line": None,
                    "end_column": None,
                },
                "related": [],
                "repair": record["repair"],
                "subject": record["subject_uri"],
            }
        )
    payload = {
        "schema_version": "0.1",
        "nodes": [_portable_node(node.to_dict()) for node in index.all_nodes()],
        "edges": [_portable_edge(edge.to_dict()) for edge in index.all_edges()],
        "diagnostics": diagnostics,
        "provenance": {
            **provenance,
            "projects": index.project_revisions(),
        },
    }
    schema_path = Path(__file__).resolve().parents[2] / "schemas/ndf-ir.schema.json"
    schema = json.loads(schema_path.read_text())
    missing = set(schema["required"]) - set(payload)
    if missing:
        raise ValueError(f"NDF export is missing schema fields: {sorted(missing)}")
    encoded = json.dumps(
        payload,
        sort_keys=True,
        indent=2,
        ensure_ascii=False,
    ) + "\n"
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(encoded)
    return ExportResult(
        output=output,
        semantic_hash=provenance["semantic_hash"],
        node_count=len(payload["nodes"]),
        edge_count=len(payload["edges"]),
    )
