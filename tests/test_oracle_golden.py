from pathlib import Path
import subprocess
import sys
import unittest


REPOSITORY = Path(__file__).resolve().parents[1]


class OracleGoldenTest(unittest.TestCase):
    def test_committed_golden_files_match_python_oracle(self) -> None:
        result = subprocess.run(
            [sys.executable, "tests/oracle/generate_golden.py", "--check"],
            cwd=REPOSITORY,
            text=True,
            capture_output=True,
            check=False,
        )

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
