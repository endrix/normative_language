import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import yaml


_COMMIT = re.compile(r"^[0-9a-f]{40}$")


class _UniqueKeyLoader(yaml.SafeLoader):
    pass


def _construct_mapping(
    loader: _UniqueKeyLoader,
    node: yaml.MappingNode,
    deep: bool = False,
) -> dict[str, Any]:
    mapping: dict[str, Any] = {}
    for key_node, value_node in node.value:
        key = loader.construct_object(key_node, deep=deep)
        if key in mapping:
            raise ValueError(f"duplicate YAML key: {key}")
        mapping[key] = loader.construct_object(value_node, deep=deep)
    return mapping


_UniqueKeyLoader.add_constructor(
    yaml.resolver.BaseResolver.DEFAULT_MAPPING_TAG,
    _construct_mapping,
)


@dataclass(frozen=True)
class ProjectManifest:
    format_version: str
    project: str
    roots: tuple[str, ...]
    id_prefixes: tuple[str, ...]
    domains: tuple[str, ...]
    policies: tuple[tuple[str, Any], ...]
    root: Path


@dataclass(frozen=True)
class LockedDependency:
    project: str
    uri: str
    revision: str
    path: str


@dataclass(frozen=True)
class ProjectLock:
    format_version: str
    dependencies: tuple[LockedDependency, ...]


def _load_mapping(path: Path) -> dict[str, Any]:
    try:
        value = yaml.load(path.read_text(), Loader=_UniqueKeyLoader)
    except yaml.YAMLError as error:
        raise ValueError(f"invalid YAML in {path}: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"expected a YAML mapping in {path}")
    return value


def _required(mapping: dict[str, Any], field: str) -> Any:
    if field not in mapping:
        raise ValueError(f"missing required field: {field}")
    return mapping[field]


def _string_tuple(value: Any, field: str) -> tuple[str, ...]:
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        raise ValueError(f"{field} must be a list of strings")
    return tuple(value)


def load_manifest(path: Path) -> ProjectManifest:
    mapping = _load_mapping(path)
    allowed = {
        "format_version",
        "project",
        "roots",
        "id_prefixes",
        "domains",
        "policies",
    }
    unknown = sorted(set(mapping) - allowed)
    if unknown:
        raise ValueError(f"unsupported manifest field: {unknown[0]}")

    format_version = _required(mapping, "format_version")
    project = _required(mapping, "project")
    roots = _string_tuple(_required(mapping, "roots"), "roots")
    id_prefixes = _string_tuple(
        _required(mapping, "id_prefixes"),
        "id_prefixes",
    )
    domains = _string_tuple(mapping.get("domains", []), "domains")
    policies = mapping.get("policies", {})
    if not isinstance(format_version, str):
        raise ValueError("format_version must be a string")
    if not isinstance(project, str):
        raise ValueError("project must be a string")
    if not isinstance(policies, dict):
        raise ValueError("policies must be a mapping")

    return ProjectManifest(
        format_version=format_version,
        project=project,
        roots=roots,
        id_prefixes=id_prefixes,
        domains=domains,
        policies=tuple(sorted(policies.items())),
        root=path.parent.resolve(),
    )


def load_lock(path: Path) -> ProjectLock:
    mapping = _load_mapping(path)
    allowed = {"format_version", "dependencies"}
    unknown = sorted(set(mapping) - allowed)
    if unknown:
        raise ValueError(f"unsupported lock field: {unknown[0]}")

    format_version = _required(mapping, "format_version")
    dependencies = _required(mapping, "dependencies")
    if not isinstance(format_version, str):
        raise ValueError("format_version must be a string")
    if not isinstance(dependencies, dict):
        raise ValueError("dependencies must be a mapping")

    normalized: list[LockedDependency] = []
    for project, value in sorted(dependencies.items()):
        if not isinstance(project, str) or not isinstance(value, dict):
            raise ValueError("each dependency must be a named mapping")
        unknown_dependency_fields = sorted(set(value) - {"uri", "revision", "path"})
        if unknown_dependency_fields:
            raise ValueError(
                f"unsupported dependency field: {unknown_dependency_fields[0]}"
            )
        uri = _required(value, "uri")
        revision = _required(value, "revision")
        dependency_path = _required(value, "path")
        if not all(isinstance(item, str) for item in (uri, revision, dependency_path)):
            raise ValueError("dependency uri, revision, and path must be strings")
        if not _COMMIT.fullmatch(revision):
            raise ValueError("dependency revision must be 40 lowercase hexadecimal characters")
        normalized.append(
            LockedDependency(
                project=project,
                uri=uri,
                revision=revision,
                path=dependency_path,
            )
        )
    return ProjectLock(format_version, tuple(normalized))
