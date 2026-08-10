import subprocess
import sys
import unittest
from pathlib import Path


class RustParityTest(unittest.TestCase):
    def test_rust_matches_every_golden_contract(self) -> None:
        repository = Path(__file__).parents[1]
        result = subprocess.run(
            [sys.executable, "tests/oracle/compare_rust.py"],
            cwd=repository,
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
