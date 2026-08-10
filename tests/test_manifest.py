import json
import tempfile
import unittest
from pathlib import Path

from ndf.manifest import load_lock, load_manifest


class ManifestTest(unittest.TestCase):
    def setUp(self) -> None:
        self.project_root = (
            Path(__file__).parent / "fixtures" / "markdown" / "project"
        )

    def test_complete_manifest_and_lock_are_normalized(self) -> None:
        manifest = load_manifest(self.project_root / "ndf.yaml")
        lock = load_lock(self.project_root / "ndf.lock")

        self.assertEqual(manifest.format_version, "0.2")
        self.assertEqual(manifest.project, "fixture")
        self.assertEqual(manifest.roots, ("docs/**/*.md",))
        self.assertEqual(manifest.id_prefixes, ("DOC", "PIPE"))
        self.assertEqual(lock.dependencies[0].project, "pto-spec")
        self.assertEqual(
            lock.dependencies[0].revision,
            "0123456789abcdef0123456789abcdef01234567",
        )

    def test_lock_rejects_non_commit_revision(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "ndf.lock"
            path.write_text(
                """format_version: '0.1'
dependencies:
  pto-spec:
    uri: https://github.com/PTO-ISA/pto-spec.git
    revision: main
    path: external/pto-spec
"""
            )

            with self.assertRaisesRegex(ValueError, "40 lowercase hexadecimal"):
                load_lock(path)

    def test_manifest_rejects_missing_required_field(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "ndf.yaml"
            path.write_text("format_version: '0.2'\nproject: fixture\n")

            with self.assertRaisesRegex(ValueError, "missing required field: roots"):
                load_manifest(path)

    def test_manifest_schemas_declare_runtime_required_fields(self) -> None:
        repository_root = Path(__file__).parents[1]
        project_schema = json.loads(
            (repository_root / "schemas/ndf-project.schema.json").read_text()
        )
        lock_schema = json.loads(
            (repository_root / "schemas/ndf-lock.schema.json").read_text()
        )
        front_matter_schema = json.loads(
            (repository_root / "schemas/ndf-front-matter.schema.json").read_text()
        )

        self.assertEqual(
            set(project_schema["required"]),
            {"format_version", "project", "roots", "id_prefixes"},
        )
        self.assertEqual(
            set(lock_schema["required"]),
            {"format_version", "dependencies"},
        )
        self.assertEqual(
            set(front_matter_schema["required"]),
            {"doc_id", "status", "authority", "owner"},
        )


if __name__ == "__main__":
    unittest.main()
