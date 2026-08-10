import json
import unittest
from contextlib import redirect_stdout
from io import StringIO

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


if __name__ == "__main__":
    unittest.main()
