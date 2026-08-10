# NDF Release Performance Reference Machine

The NDF `v0.1.0` release-scale contract uses the following reference system:

- Apple M5 (`arm64`)
- 32 GiB physical memory
- macOS 26.5.2 (Darwin 25.5.0)
- Rust 1.94.0, release profile, locked dependencies

The workload materializes the same deterministic graph twice. Each build contains
100,000 nodes and 1,000,000 edges. Both builds must complete in less than 10
seconds, produce byte-identical SQLite databases and semantic hashes, and keep
peak resident memory below 536,870,912 bytes.

The 2026-08-10 release-candidate measurement completed the two builds in 1.459
seconds and 1.457 seconds. Peak resident memory was 326,434,816 bytes, and the
materialized SQLite index was 239,931,392 bytes.

Run the complete measurement with:

```bash
scripts/check-release-performance
```

The script uses `/usr/bin/time -l` on macOS and `/usr/bin/time -v` on Linux. CI
keeps correctness and determinism as hard gates; this reference-machine check is
the release gate because shared CI timing is noisy.
