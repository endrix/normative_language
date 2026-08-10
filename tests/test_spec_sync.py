import unittest
from pathlib import Path

import yaml


class SpecificationContractTest(unittest.TestCase):
    def test_english_and_chinese_publish_the_same_machine_contract(self) -> None:
        repository_root = Path(__file__).parents[1]
        english = self._contract(repository_root / "normative_language.md")
        chinese = self._contract(repository_root / "normative_language_cn.md")

        self.assertEqual(english, chinese)
        self.assertEqual(
            english,
            {
                "agent_gate": ["build", "check", "diff", "coverage"],
                "canonical_fields": [
                    "kind",
                    "modality",
                    "refinement",
                    "domain",
                    "status",
                ],
                "derived_index": "sqlite-v1",
                "format_version": "0.2",
                "identity_scheme": "ndf",
                "ir_version": "0.1",
                "machine_output_schema": "0.1",
                "maturity": {
                    "cli": "experimental",
                    "format": "experimental",
                    "ir": "experimental",
                    "plugin_api": "experimental",
                },
                "required_files": ["ndf.yaml", "ndf.lock"],
            },
        )

    @staticmethod
    def _contract(path: Path) -> dict:
        text = path.read_text()
        start_marker = "<!-- ndf:format-contract:start -->"
        end_marker = "<!-- ndf:format-contract:end -->"
        start = text.index(start_marker) + len(start_marker)
        end = text.index(end_marker, start)
        block = text[start:end].strip()
        if not block.startswith("```yaml\n") or not block.endswith("\n```"):
            raise AssertionError(f"{path.name} contract must be a YAML code block")
        value = yaml.safe_load(block[len("```yaml\n") : -len("\n```")])
        if not isinstance(value, dict):
            raise AssertionError(f"{path.name} contract must be a mapping")
        return value


if __name__ == "__main__":
    unittest.main()
