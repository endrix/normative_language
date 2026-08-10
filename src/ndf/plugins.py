from dataclasses import dataclass
from typing import Any

from .manifest import ProjectManifest


@dataclass(frozen=True)
class Rule:
    namespace: str
    name: str
    value: Any


def load_project_rules(manifest: ProjectManifest) -> tuple[Rule, ...]:
    rules: list[Rule] = []
    for namespace, configuration in manifest.policies:
        if namespace != "ndf":
            raise ValueError(
                f"NDF-PLUGIN-001: unsupported rule namespace: {namespace}"
            )
        if not isinstance(configuration, dict):
            raise ValueError(
                "NDF-PLUGIN-001: rule namespace configuration must be a mapping"
            )
        for name, value in sorted(configuration.items()):
            rules.append(Rule(namespace, name, value))
    return tuple(rules)
