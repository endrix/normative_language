import re
from collections.abc import Iterable


_PREFIX = re.compile(r"^[A-Z][A-Z0-9-]*[A-Z0-9]$")


def allocate_id(prefix: str, occupied: Iterable[str]) -> str:
    if not _PREFIX.fullmatch(prefix):
        raise ValueError(f"invalid ID prefix: {prefix!r}")
    suffixes: list[int] = []
    pattern = re.compile(rf"^{re.escape(prefix)}-(\d+)$")
    for value in occupied:
        match = pattern.fullmatch(value)
        if match:
            suffixes.append(int(match.group(1)))
    next_suffix = max(suffixes, default=0) + 1
    return f"{prefix}-{next_suffix:03d}"
