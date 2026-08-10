from __future__ import annotations

import json
import sqlite3
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


REPOSITORY = Path(__file__).parents[2]
RUST_NDF = REPOSITORY / "target" / "release" / "ndf"
TABLES = (
    "projects",
    "sources",
    "nodes",
    "node_attrs",
    "aliases",
    "edges",
    "diagnostics",
    "baselines",
    "builds",
)


def command(arguments: list[str], cwd: Path) -> tuple[int, Any, str]:
    result = subprocess.run(
        arguments,
        cwd=cwd,
        text=True,
        capture_output=True,
        check=False,
    )
    try:
        payload = json.loads(result.stdout)
    except json.JSONDecodeError:
        payload = None
    return result.returncode, payload, result.stdout + result.stderr


def logical_rows(path: Path) -> dict[str, list[list[Any]]]:
    with sqlite3.connect(path) as connection:
        return {
            table: [list(row) for row in sorted(connection.execute(f"SELECT * FROM {table}"))]
            for table in TABLES
        }


def main() -> int:
    failures: list[str] = []

    build = subprocess.run(
        ["cargo", "build", "--workspace", "--release", "--locked"],
        cwd=REPOSITORY,
        text=True,
        capture_output=True,
        check=False,
    )
    if build.returncode:
        print(build.stdout + build.stderr)
        return build.returncode

    golden = json.loads((REPOSITORY / "tests/golden/machine-version.json").read_text())
    status, payload, output = command([str(RUST_NDF), "--machine-version"], REPOSITORY)
    if status != golden["exit_code"] or payload != golden["payload"]:
        failures.append(f"machine-version: {output}")

    golden_tests = {
        "identity": ["cargo", "test", "-p", "ndf-core", "--test", "identity", "--locked"],
        "markdown-valid/invalid": [
            "cargo",
            "test",
            "-p",
            "ndf-compiler",
            "--test",
            "markdown",
            "--locked",
        ],
        "pto-asl": [
            "cargo",
            "test",
            "-p",
            "ndf-compiler",
            "--test",
            "pto_asl",
            "--locked",
        ],
        "index": [
            "cargo",
            "test",
            "-p",
            "ndf-compiler",
            "--test",
            "index",
            "--locked",
        ],
        "query-report": [
            "cargo",
            "test",
            "-p",
            "ndf-compiler",
            "--test",
            "query",
            "--locked",
        ],
    }
    for name, arguments in golden_tests.items():
        result = subprocess.run(
            arguments,
            cwd=REPOSITORY,
            text=True,
            capture_output=True,
            check=False,
        )
        if result.returncode:
            failures.append(f"{name}: {result.stdout}{result.stderr}")

    fixture = REPOSITORY / "tests/fixtures/markdown/project"
    with tempfile.TemporaryDirectory() as directory:
        temporary = Path(directory)
        implementations = {
            "python": [sys.executable, "-m", "ndf"],
            "rust": [str(RUST_NDF)],
        }
        roots = {name: temporary / name for name in implementations}
        for root in roots.values():
            root.mkdir()
        results: dict[str, dict[str, tuple[int, Any, str]]] = {
            name: {} for name in implementations
        }
        commands = {
            "build": [
                "build",
                "--root",
                str(fixture),
                "--output",
                "graph.sqlite",
                "--format",
                "json",
            ],
            "check": ["check", "--root", str(fixture), "--format", "json"],
            "show": [
                "show",
                "ndf://fixture/PIPE-ISSUE-001",
                "--format",
                "json",
            ],
            "trace": [
                "trace",
                "PIPE-ISSUE-010",
                "--edge",
                "refines",
                "--format",
                "json",
            ],
            "coverage": ["report", "coverage", "--format", "json"],
            "diff": [
                "diff",
                "graph.sqlite",
                "graph.sqlite",
                "--format",
                "json",
            ],
            "id": ["id", "allocate", "PIPE-ISSUE", "--format", "json"],
            "export": [
                "export",
                "--index",
                "graph.sqlite",
                "--output",
                "graph.json",
                "--format",
                "ndf-json",
            ],
        }
        for name, prefix in implementations.items():
            for operation, arguments in commands.items():
                results[name][operation] = command(prefix + arguments, roots[name])

        for operation in commands:
            python_status, python_payload, python_output = results["python"][operation]
            rust_status, rust_payload, rust_output = results["rust"][operation]
            if (python_status, python_payload) != (rust_status, rust_payload):
                failures.append(
                    f"CLI {operation}:\nPYTHON {python_output}\nRUST {rust_output}"
                )

        python_rows = logical_rows(roots["python"] / "graph.sqlite")
        rust_rows = logical_rows(roots["rust"] / "graph.sqlite")
        if python_rows != rust_rows:
            failures.append("SQLite logical rows differ")

        python_export = json.loads((roots["python"] / "graph.json").read_text())
        rust_export = json.loads((roots["rust"] / "graph.json").read_text())
        if python_export != rust_export:
            failures.append("exported NDF JSON differs")

    if failures:
        print("Rust parity gaps:")
        for failure in failures:
            print(f"- {failure}")
        return 1
    print("Rust parity: all frozen contracts, CLI envelopes, hashes, and SQLite rows match")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
