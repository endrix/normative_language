import tempfile
import unittest
from pathlib import Path

from ndf.fix import FixMode, safe_fix


class SafeFixTest(unittest.TestCase):
    def test_check_mode_reports_metadata_reordering_without_writing(self) -> None:
        content = "<!-- ndf: status=active kind=requirement modality=must -->\n"
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "design.md"
            path.write_text(content)

            result = safe_fix((path,), FixMode(check=True))

            self.assertEqual(result.changed, (str(path),))
            self.assertEqual(path.read_text(), content)
            self.assertEqual(result.diagnostics, ())

    def test_fix_reorders_metadata_without_changing_values(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "design.md"
            path.write_text(
                "<!-- ndf: status=active kind=requirement modality=must -->\n"
            )

            result = safe_fix((path,), FixMode())

            self.assertEqual(result.changed, (str(path),))
            self.assertEqual(
                path.read_text(),
                "<!-- ndf: kind=requirement modality=must status=active -->\n",
            )

    def test_semantic_change_request_is_refused(self) -> None:
        result = safe_fix(
            (),
            FixMode(requested_changes=("status",)),
        )

        self.assertEqual([item.code for item in result.diagnostics], ["NDF-FIX-001"])


if __name__ == "__main__":
    unittest.main()
