import re
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path

from .model import Diagnostic, DiagnosticSeverity, SourceSpan


_METADATA = re.compile(r"^(\s*)<!-- ndf:\s*(.*?)\s*-->(\s*)$", re.MULTILINE)
_SEMANTIC_FIELDS = {
    "prose",
    "authority",
    "status",
    "kind",
    "modality",
    "refinement",
    "domain",
    "edges",
}


@dataclass(frozen=True)
class FixMode:
    generated_only: bool = False
    check: bool = False
    requested_changes: tuple[str, ...] = ()


@dataclass(frozen=True)
class FixResult:
    changed: tuple[str, ...]
    diagnostics: tuple[Diagnostic, ...]

    def to_dict(self) -> dict:
        return {
            "changed": list(self.changed),
            "diagnostics": [item.to_dict() for item in self.diagnostics],
        }


def _refusal(fields: set[str]) -> Diagnostic:
    return Diagnostic(
        code="NDF-FIX-001",
        severity=DiagnosticSeverity.ERROR,
        message=f"safe fix cannot alter semantic fields: {', '.join(sorted(fields))}",
        primary=SourceSpan("<command>", 1),
        repair="Edit normative semantics explicitly and request review.",
    )


def _sort_metadata(match: re.Match[str]) -> str:
    tokens = match.group(2).split()
    return f"{match.group(1)}<!-- ndf: {' '.join(sorted(tokens))} -->{match.group(3)}"


def _sort_generated_blocks(content: str) -> str:
    start_marker = "<!-- ndf:index:start -->"
    end_marker = "<!-- ndf:index:end -->"
    position = 0
    pieces: list[str] = []
    while True:
        start = content.find(start_marker, position)
        if start < 0:
            pieces.append(content[position:])
            break
        end = content.find(end_marker, start + len(start_marker))
        if end < 0:
            pieces.append(content[position:])
            break
        body_start = start + len(start_marker)
        body = content[body_start:end]
        lines = sorted(line for line in body.splitlines() if line.strip())
        pieces.append(content[position:body_start])
        pieces.append("\n" + "\n".join(lines) + "\n")
        position = end
    return "".join(pieces)


def safe_fix(paths: Sequence[Path], mode: FixMode) -> FixResult:
    forbidden = set(mode.requested_changes) & _SEMANTIC_FIELDS
    if forbidden:
        return FixResult((), (_refusal(forbidden),))

    changed: list[str] = []
    for path in sorted(paths):
        content = path.read_text()
        updated = _sort_generated_blocks(content)
        if not mode.generated_only:
            updated = _METADATA.sub(_sort_metadata, updated)
        if updated == content:
            continue
        changed.append(str(path))
        if not mode.check:
            path.write_text(updated)
    return FixResult(tuple(changed), ())
