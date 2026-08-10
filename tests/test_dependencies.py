import json
import subprocess
import tempfile
import unittest
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path

from ndf.cli import main
from ndf.dependencies import resolve_dependencies
from ndf.manifest import LockedDependency, ProjectLock, ProjectManifest


class DependencyResolutionTest(unittest.TestCase):
    def test_exact_clean_git_revision_is_resolved(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            revision = self._git_repo(root / "deps" / "tool")
            manifest, lock = self._contracts(root, revision)

            resolved = resolve_dependencies(manifest, lock, root)

            self.assertEqual(len(resolved), 1)
            self.assertEqual(resolved[0].project_id, "tool")
            self.assertEqual(resolved[0].revision, revision)
            self.assertEqual(resolved[0].dirty, False)

    def test_missing_dependency_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest, lock = self._contracts(root, "0" * 40)

            with self.assertRaisesRegex(ValueError, "NDF-DEP-001"):
                resolve_dependencies(manifest, lock, root)

    def test_symbolic_revision_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self._git_repo(root / "deps" / "tool")
            manifest, _ = self._contracts(root, "0" * 40)
            lock = ProjectLock(
                "0.1",
                (LockedDependency("tool", "https://example.invalid/tool.git", "main", "deps/tool"),),
            )

            with self.assertRaisesRegex(ValueError, "NDF-DEP-002"):
                resolve_dependencies(manifest, lock, root)

    def test_revision_mismatch_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self._git_repo(root / "deps" / "tool")
            manifest, lock = self._contracts(root, "f" * 40)

            with self.assertRaisesRegex(ValueError, "NDF-DEP-003"):
                resolve_dependencies(manifest, lock, root)

    def test_dirty_dependency_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            dependency = root / "deps" / "tool"
            revision = self._git_repo(dependency)
            (dependency / "dirty.txt").write_text("dirty\n")
            manifest, lock = self._contracts(root, revision)

            with self.assertRaisesRegex(ValueError, "NDF-DEP-004"):
                resolve_dependencies(manifest, lock, root)

    def test_cli_reports_resolved_compatibility_tuple(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            revision = self._git_repo(root / "deps" / "tool")
            (root / "ndf.yaml").write_text(
                """format_version: '0.2'
project: fixture
roots: []
id_prefixes: [REQ]
dependencies:
  tool:
    path: deps/tool
"""
            )
            (root / "ndf.lock").write_text(
                f"""format_version: '0.1'
dependencies:
  tool:
    uri: https://example.invalid/tool.git
    revision: {revision}
    path: deps/tool
"""
            )
            output = StringIO()

            with redirect_stdout(output):
                status = main(
                    [
                        "report",
                        "dependencies",
                        "--root",
                        str(root),
                        "--format",
                        "json",
                    ]
                )

            payload = json.loads(output.getvalue())
            self.assertEqual(status, 0)
            self.assertEqual(payload["command"], "report dependencies")
            self.assertEqual(payload["data"][0]["revision"], revision)

    @staticmethod
    def _contracts(root: Path, revision: str) -> tuple[ProjectManifest, ProjectLock]:
        manifest = ProjectManifest(
            format_version="0.2",
            project="fixture",
            roots=(),
            id_prefixes=("REQ",),
            domains=(),
            policies=(),
            root=root,
            dependencies=("tool",),
        )
        lock = ProjectLock(
            "0.1",
            (
                LockedDependency(
                    "tool",
                    "https://example.invalid/tool.git",
                    revision,
                    "deps/tool",
                ),
            ),
        )
        return manifest, lock

    @staticmethod
    def _git_repo(path: Path) -> str:
        path.mkdir(parents=True)
        subprocess.run(["git", "init", "-q"], cwd=path, check=True)
        subprocess.run(["git", "config", "user.email", "ndf@example.invalid"], cwd=path, check=True)
        subprocess.run(["git", "config", "user.name", "NDF Test"], cwd=path, check=True)
        (path / "README.md").write_text("fixture\n")
        subprocess.run(["git", "add", "README.md"], cwd=path, check=True)
        subprocess.run(["git", "commit", "-qm", "fixture"], cwd=path, check=True)
        return subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=path,
            check=True,
            text=True,
            capture_output=True,
        ).stdout.strip()


if __name__ == "__main__":
    unittest.main()
