import hashlib
import re
from dataclasses import dataclass
from datetime import date
from pathlib import PurePosixPath
from collections.abc import Iterable

from .identity import parse_uri
from .model import Diagnostic, DiagnosticSeverity, SourceSpan


_WHITESPACE = re.compile(r"\s+")
_SEVERITY_RANK = {
    DiagnosticSeverity.INFORMATION: 0,
    DiagnosticSeverity.WARNING: 1,
    DiagnosticSeverity.ERROR: 2,
}


def _normalized_path(value: str) -> str:
    return PurePosixPath(value.replace("\\", "/")).as_posix()


def diagnostic_fingerprint(diagnostic: Diagnostic) -> str:
    payload = "\0".join(
        (
            diagnostic.code,
            str(diagnostic.subject) if diagnostic.subject else "",
            _normalized_path(diagnostic.primary.path),
            _WHITESPACE.sub(" ", diagnostic.message).strip(),
        )
    )
    return hashlib.sha256(payload.encode()).hexdigest()


@dataclass(frozen=True)
class BaselineEntry:
    fingerprint: str
    code: str
    severity: DiagnosticSeverity
    subject: str
    path: str
    owner: str | None
    expires: str | None

    @classmethod
    def from_diagnostic(
        cls,
        diagnostic: Diagnostic,
        *,
        owner: str,
        expires: str,
    ) -> "BaselineEntry":
        return cls(
            fingerprint=diagnostic_fingerprint(diagnostic),
            code=diagnostic.code,
            severity=diagnostic.severity,
            subject=str(diagnostic.subject) if diagnostic.subject else "",
            path=_normalized_path(diagnostic.primary.path),
            owner=owner,
            expires=expires,
        )


@dataclass(frozen=True)
class DiagnosticBaseline:
    entries: tuple[BaselineEntry, ...] = ()


@dataclass(frozen=True)
class BaselineResult:
    existing: tuple[Diagnostic, ...] = ()
    new: tuple[Diagnostic, ...] = ()
    worsened: tuple[Diagnostic, ...] = ()
    expired: tuple[BaselineEntry, ...] = ()


def _integrity_diagnostic(entry: BaselineEntry) -> Diagnostic:
    subject = parse_uri(entry.subject) if entry.subject else None
    return Diagnostic(
        code="NDF-BASE-003",
        severity=DiagnosticSeverity.ERROR,
        message=f"baseline entry {entry.fingerprint} lacks owner or expiry",
        primary=SourceSpan(entry.path or "ndf-baseline.json", 1),
        repair="Assign an owner and ISO expiry date to the baseline entry.",
        subject=subject,
    )


def compare_baseline(
    current: Iterable[Diagnostic],
    baseline: DiagnosticBaseline,
) -> BaselineResult:
    by_fingerprint: dict[str, BaselineEntry] = {}
    invalid_fingerprints: set[str] = set()
    integrity: list[Diagnostic] = []
    expired: list[BaselineEntry] = []
    today = date.today()

    for entry in baseline.entries:
        if not entry.owner or not entry.expires:
            invalid_fingerprints.add(entry.fingerprint)
            integrity.append(_integrity_diagnostic(entry))
            continue
        try:
            expiry = date.fromisoformat(entry.expires)
        except ValueError:
            invalid_fingerprints.add(entry.fingerprint)
            integrity.append(_integrity_diagnostic(entry))
            continue
        if expiry < today:
            expired.append(entry)
        by_fingerprint[entry.fingerprint] = entry

    existing: list[Diagnostic] = []
    new: list[Diagnostic] = list(integrity)
    worsened: list[Diagnostic] = []
    for diagnostic in current:
        fingerprint = diagnostic_fingerprint(diagnostic)
        if fingerprint in invalid_fingerprints:
            continue
        entry = by_fingerprint.get(fingerprint)
        if entry is None:
            new.append(diagnostic)
        elif _SEVERITY_RANK[diagnostic.severity] > _SEVERITY_RANK[entry.severity]:
            worsened.append(diagnostic)
        else:
            existing.append(diagnostic)

    return BaselineResult(
        existing=tuple(existing),
        new=tuple(new),
        worsened=tuple(worsened),
        expired=tuple(expired),
    )
