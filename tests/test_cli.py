import json
import tempfile
import unittest
from contextlib import chdir
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path

from ndf.cli import main


class CliVersionTest(unittest.TestCase):
    def test_machine_version_has_independent_surfaces(self) -> None:
        output = StringIO()
        with redirect_stdout(output):
            status = main(["--machine-version"])

        self.assertEqual(status, 0)
        self.assertEqual(
            json.loads(output.getvalue()),
            {
                "cli_stability": "experimental",
                "format_version": "0.2",
                "ir_version": "0.1",
                "plugin_api": "experimental",
                "tool_version": "0.1.0",
            },
        )

    def test_experimental_commands_emit_versioned_machine_envelopes(self) -> None:
        fixture = Path(__file__).parent / "fixtures" / "markdown" / "project"
        with tempfile.TemporaryDirectory() as directory, chdir(directory):
            commands = (
                (
                    [
                        "build",
                        "--root",
                        str(fixture),
                        "--output",
                        "graph.sqlite",
                        "--format",
                        "json",
                    ],
                    "build",
                ),
                (["check", "--root", str(fixture), "--format", "json"], "check"),
                (
                    [
                        "show",
                        "ndf://fixture/PIPE-ISSUE-001",
                        "--index",
                        "graph.sqlite",
                        "--format",
                        "json",
                    ],
                    "show",
                ),
                (["trace", "PIPE-ISSUE-010", "--edge", "refines", "--format", "json"], "trace"),
                (["report", "coverage", "--format", "json"], "report coverage"),
                (["diff", "graph.sqlite", "graph.sqlite", "--format", "json"], "diff"),
                (
                    [
                        "export",
                        "--index",
                        "graph.sqlite",
                        "--output",
                        "graph.json",
                        "--format",
                        "ndf-json",
                    ],
                    "export",
                ),
                (["id", "allocate", "PIPE-ISSUE", "--format", "json"], "id allocate"),
                (["fix", "--generated-only", "--check"], "fix"),
            )

            for arguments, command in commands:
                with self.subTest(command=command):
                    output = StringIO()
                    with redirect_stdout(output):
                        status = main(arguments)
                    payload = json.loads(output.getvalue())
                    self.assertEqual(status, 0)
                    self.assertEqual(
                        set(payload),
                        {"schema_version", "command", "ok", "data", "diagnostics"},
                    )
                    self.assertEqual(payload["schema_version"], "0.1")
                    self.assertEqual(payload["command"], command)
                    self.assertEqual(payload["ok"], True)


if __name__ == "__main__":
    unittest.main()
