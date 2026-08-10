import unittest

from ndf.identity import NodeId, canonical_uri, parse_uri


class IdentityTest(unittest.TestCase):
    def test_local_id_resolves_inside_current_project(self) -> None:
        self.assertEqual(
            str(parse_uri("VEC-ISSUE-001", current_project="davincioo")),
            "ndf://davincioo/VEC-ISSUE-001",
        )

    def test_cross_project_reference_requires_uri(self) -> None:
        with self.assertRaisesRegex(ValueError, "current project is required"):
            parse_uri("VEC-ISSUE-001")

    def test_canonical_uri_round_trips(self) -> None:
        identity = parse_uri("ndf://pto-spec/PTO-INST-TLOAD")

        self.assertEqual(identity, NodeId("pto-spec", "PTO-INST-TLOAD"))
        self.assertEqual(
            canonical_uri(identity.project_id, identity.local_id),
            "ndf://pto-spec/PTO-INST-TLOAD",
        )

    def test_invalid_project_id_is_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "invalid project ID"):
            canonical_uri("DavinciOO", "PIPE-001")


if __name__ == "__main__":
    unittest.main()
