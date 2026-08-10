import unittest

from ndf.id_allocator import allocate_id


class IdAllocatorTest(unittest.TestCase):
    def test_allocates_first_available_monotonic_suffix(self) -> None:
        self.assertEqual(
            allocate_id("PIPE-ISSUE", ("PIPE-ISSUE-001", "PIPE-ISSUE-003")),
            "PIPE-ISSUE-004",
        )

    def test_rejects_invalid_prefix(self) -> None:
        with self.assertRaisesRegex(ValueError, "invalid ID prefix"):
            allocate_id("pipe issue", ())


if __name__ == "__main__":
    unittest.main()
