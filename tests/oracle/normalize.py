import json
from pathlib import Path
from typing import Any


def normalize_value(value: Any, repository: Path) -> Any:
    if isinstance(value, dict):
        return {
            key: normalize_value(item, repository)
            for key, item in sorted(value.items())
        }
    if isinstance(value, (list, tuple)):
        return [normalize_value(item, repository) for item in value]
    if isinstance(value, str):
        path = Path(value)
        if path.is_absolute():
            try:
                return path.resolve().relative_to(repository.resolve()).as_posix()
            except ValueError:
                return value
        return value.replace("\\", "/")
    return value


def normalized_json(value: Any, repository: Path) -> str:
    normalized = normalize_value(value, repository)
    return json.dumps(normalized, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
