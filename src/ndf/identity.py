import re
from dataclasses import dataclass
from urllib.parse import urlsplit


_PROJECT_ID = re.compile(r"^[a-z0-9][a-z0-9._-]*$")
_LOCAL_ID = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._:-]*$")


def _validate_project_id(value: str) -> None:
    if not _PROJECT_ID.fullmatch(value):
        raise ValueError(f"invalid project ID: {value!r}")


def _validate_local_id(value: str) -> None:
    if not _LOCAL_ID.fullmatch(value):
        raise ValueError(f"invalid local ID: {value!r}")


def canonical_uri(project_id: str, local_id: str) -> str:
    _validate_project_id(project_id)
    _validate_local_id(local_id)
    return f"ndf://{project_id}/{local_id}"


@dataclass(frozen=True, order=True)
class NodeId:
    project_id: str
    local_id: str

    def __post_init__(self) -> None:
        _validate_project_id(self.project_id)
        _validate_local_id(self.local_id)

    def __str__(self) -> str:
        return canonical_uri(self.project_id, self.local_id)


def parse_uri(value: str, current_project: str | None = None) -> NodeId:
    if not value.startswith("ndf://"):
        if current_project is None:
            raise ValueError("current project is required for a local NDF ID")
        return NodeId(current_project, value)

    parsed = urlsplit(value)
    if parsed.scheme != "ndf" or not parsed.netloc:
        raise ValueError(f"invalid NDF URI: {value!r}")
    if parsed.query or parsed.fragment:
        raise ValueError(f"invalid NDF URI: {value!r}")
    local_id = parsed.path.removeprefix("/")
    if not local_id or "/" in local_id:
        raise ValueError(f"invalid NDF URI: {value!r}")
    return NodeId(parsed.netloc, local_id)
