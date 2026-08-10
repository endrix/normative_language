import re
import subprocess
from dataclasses import dataclass
from pathlib import Path

from .manifest import ProjectLock, ProjectManifest


_REVISION = re.compile(r"^[0-9a-f]{40}$")


@dataclass(frozen=True)
class ResolvedProject:
    project_id: str
    path: Path
    revision: str
    dirty: bool

    def to_dict(self) -> dict:
        return {
            "project_id": self.project_id,
            "path": self.path.as_posix(),
            "revision": self.revision,
            "dirty": self.dirty,
        }


def _git(path: Path, *arguments: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *arguments],
        cwd=path,
        text=True,
        capture_output=True,
        check=False,
    )


def resolve_dependencies(
    manifest: ProjectManifest,
    lock: ProjectLock,
    workspace: Path,
) -> tuple[ResolvedProject, ...]:
    locked = {dependency.project: dependency for dependency in lock.dependencies}
    policy = dict(manifest.policies).get("ndf", {})
    allow_dirty = bool(policy.get("allow_dirty", False)) if isinstance(policy, dict) else False
    resolved: list[ResolvedProject] = []
    for project in sorted(manifest.dependencies):
        dependency = locked.get(project)
        if dependency is None:
            raise ValueError(f"NDF-DEP-001: dependency is missing from lock: {project}")
        if not _REVISION.fullmatch(dependency.revision):
            raise ValueError(f"NDF-DEP-002: dependency revision is not an exact commit: {project}")
        path = (workspace / dependency.path).resolve()
        if not path.is_dir():
            raise ValueError(f"NDF-DEP-001: dependency checkout is missing: {project}")
        revision_result = _git(path, "rev-parse", "HEAD")
        if revision_result.returncode != 0:
            raise ValueError(f"NDF-DEP-001: dependency is not a Git checkout: {project}")
        revision = revision_result.stdout.strip()
        if revision != dependency.revision:
            raise ValueError(
                f"NDF-DEP-003: dependency revision mismatch for {project}: "
                f"expected {dependency.revision}, got {revision}"
            )
        dirty = bool(_git(path, "status", "--porcelain").stdout.strip())
        if dirty and not allow_dirty:
            raise ValueError(f"NDF-DEP-004: dependency checkout is dirty: {project}")
        resolved.append(ResolvedProject(project, path, revision, dirty))
    return tuple(resolved)
