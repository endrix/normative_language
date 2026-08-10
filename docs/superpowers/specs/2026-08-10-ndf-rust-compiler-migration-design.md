# NDF Rust Compiler Migration Design

**Status:** Accepted
**Date:** 2026-08-10
**Decision:** Replace the merged Python prototype with a standalone Rust implementation before the first NDF release or downstream pin.

## Summary

NDF will ship as a Rust knowledge compiler. The merged Python implementation at
commit `b0df8a0e9d95ed1946204606392049d2ef3133c1` defines the prototype behavior,
but it will not become a supported runtime. During migration, it serves only as
a semantic oracle for fixtures and parity tests. After the Rust implementation
matches the accepted contracts, the Python package and its runtime dependencies
will be removed.

The first `v0.1.0` tag will point to the merged Rust release commit. PTO-SPEC and
DavinciOO must not pin NDF before that tag exists.

## Motivation

The Python prototype proves the NDF data model, diagnostics, SQLite schema, and
cross-repository workflow. It also reveals two scale risks:

- immutable tuple-based graph construction becomes quadratic as the corpus grows;
- the million-edge index test peaks near 1.23 GB because the full graph and its
  serialized forms coexist in memory.

These are primarily data-flow problems, but the repository is still early enough
to choose a compiled implementation without a compatibility migration for users.
Rust provides predictable memory ownership, efficient streaming, one distributable
binary, and a stronger foundation for future parallel parsing and graph analysis.

## Goals

- Preserve NDF format `0.2`, canonical IR `0.1`, machine-output schema `0.1`,
  diagnostic codes, source spans, and deterministic semantic hashes.
- Provide one standalone `ndf` binary with no Python runtime dependency.
- Scale to at least 100,000 nodes and 1,000,000 edges without materializing
  avoidable duplicate graph representations.
- Keep PTO-SPEC ASL authoritative for PTO architecture and DavinciOO NDF clauses
  authoritative for DavinciOO microarchitecture.
- Keep adapters and rules extensible without committing to an unstable dynamic
  library ABI.
- Preserve exact, reproducible downstream pins through Git submodules and
  `Cargo.lock`.

## Non-goals

- Do not redesign the NDF source format, canonical fields, identity rules, or
  authority model during the language migration.
- Do not keep supported Python bindings or a PyO3 compatibility layer.
- Do not introduce a background service, hosted database, or writable SQLite API.
- Do not introduce runtime-loaded Rust dynamic libraries in `v0.1.0`.
- Do not publish the Python prototype as an NDF release.

## Considered Approaches

### Standalone Rust compiler

This is the selected approach. Rust owns parsing, validation, indexing, querying,
and the CLI. Python exists only long enough to generate and verify golden
behavior. This approach minimizes long-term runtime and maintenance surfaces.

### Rust core with PyO3 bindings

This would preserve Python imports while accelerating graph and SQLite work. It
was rejected because no downstream user depends on the Python API yet, and PyO3
would add packaging, ABI, and dual-language compatibility work before it creates
user value.

### Rust indexer with a Python front end

This would migrate the measured hot path first. It was rejected because parsing,
rules, CLI error handling, and adapter behavior would remain split across two
languages, making later extension and release packaging harder.

## Architecture

Use one Cargo workspace with three initial crates:

| Crate | Responsibility |
| --- | --- |
| `ndf-core` | Canonical identities, nodes, edges, diagnostics, source spans, graph builder, serialization, and version surfaces |
| `ndf-compiler` | Manifests, locks, Markdown and ASL adapters, generic rules, baselines, SQLite indexing, queries, reports, diff, export, and safe fixes |
| `ndf-cli` | Command parsing, stable JSON envelope, process exit codes, and the `ndf` binary |

The workspace retains the conceptual boundaries described by the cross-repository
design without creating one crate per module. Split another crate only when an
independent consumer or dependency boundary requires it.

The repository keeps JSON Schemas and SQL migrations outside crate internals:

```text
Cargo.toml
Cargo.lock
rust-toolchain.toml
crates/
    ndf-core/
    ndf-compiler/
    ndf-cli/
schemas/
sql/
tests/
    fixtures/
    golden/
```

Pin Rust `1.94.0` in `rust-toolchain.toml`. Commit `Cargo.lock`. Use a small,
auditable dependency set: Serde for contracts, Clap for the CLI, a YAML parser
for manifests, SHA-256 for semantic hashes, and SQLite with a reproducible bundled
build. Record exact dependency selections in the implementation plan.

## Data Flow and Memory Model

Parsing writes into a mutable `GraphBuilder` backed by maps and sets keyed by
canonical URI. Duplicate detection is expected constant time. The builder freezes
into deterministically ordered records only at an API boundary that requires a
complete graph.

Index construction uses one SQLite transaction and prepared batch inserts. It
streams sorted records where possible and computes the semantic hash incrementally
from canonical serialization. Query, coverage, trace, and diff operations push
filtering and traversal into SQL instead of loading every row into Rust.

JSON export writes through a buffered serializer. It must not assemble the full
export document as a second in-memory graph.

## Compatibility Boundary

The Rust migration preserves these externally observable contracts:

- `ndf --machine-version` field names and values;
- command names, required arguments, exit-code meanings, and the versioned JSON
  response envelope;
- canonical `ndf://<project>/<local-id>` identity parsing;
- serialized node, edge, diagnostic, report, diff, and export field names;
- stable diagnostic codes, severities, primary spans, related spans, and repairs;
- deterministic node and edge ordering;
- SQLite schema version `1`, query results, provenance, and semantic hash;
- legacy DavinciOO Markdown mappings and PTO ASL adapter mappings.

The CLI and plugin maturity remain experimental. Deliberate contract changes
still require a fixture change, release note, and explicit version decision; they
must not appear accidentally as a consequence of the rewrite.

## Extension Model

Rust traits define internal parser, adapter, rule, and report boundaries. The
`v0.1.0` binary registers built-in adapters statically from the project manifest.
This avoids unstable Rust dynamic-library ABI commitments.

When an independently released adapter is needed, NDF will add a versioned
JSON/NDJSON subprocess protocol. That protocol can support Rust or other
languages without loading foreign code into the compiler process. It is deferred
until a real external adapter supplies its requirements.

## Error Handling

Expected source and policy failures become structured NDF diagnostics and return
exit status `1`. Invalid invocations return a JSON error envelope when machine
output is requested and a concise CLI error otherwise. Internal failures carry
context through typed Rust errors and never produce a panic backtrace as the
normal user interface.

The compiler writes SQLite output atomically through a temporary file in the
destination directory. A failed build leaves the previous valid index untouched.

## Migration Sequence

1. Freeze Python behavior into language-neutral fixtures and JSON golden files.
2. Add the Rust workspace and reproduce machine-version and identity contracts.
3. Port manifests, locks, Markdown parsing, graph validation, and baselines.
4. Port deterministic SQLite indexing, queries, coverage, trace, diff, and export.
5. Port PTO ASL, repository, test, evidence, and dependency adapters.
6. Reproduce the complete CLI and run Python-versus-Rust parity over all fixtures.
7. Meet correctness, determinism, and scale gates in release mode.
8. Delete the Python package, PyYAML lock, Python-specific CI, and compatibility
   wrappers.
9. Merge the Rust implementation and tag the exact merge commit as `v0.1.0`.
10. Pin `v0.1.0` in PTO-SPEC, then continue the existing upstream-to-downstream
    migration into DavinciOO.

Python parity tests run during steps 1 through 7. Step 8 replaces them with
language-neutral golden tests so future development does not require Python.
The original prototype remains recoverable from Git history.

## Verification Strategy

Every feature is ported with test-driven development. A Rust test first consumes
an existing fixture or a newly frozen Python result and fails for the missing Rust
behavior. The minimal Rust implementation then makes it pass.

Release verification includes:

- unit tests for identities, canonical model invariants, parsers, rules, and
  query algorithms;
- integration tests for every CLI command and JSON envelope;
- golden parity for accepted output, rejected input, diagnostics, source spans,
  semantic hashes, and SQLite query results;
- two-build determinism from identical locked sources;
- an opt-in release benchmark with 100,000 nodes and 1,000,000 edges;
- clean-checkout end-to-end tests for the bundled Markdown and PTO ASL fixtures.

The release benchmark must finish in less than 10 seconds and peak below 512 MiB
on the documented reference machine. CI records timing and size evidence but does
not use a noisy microbenchmark to reject ordinary pull requests. Determinism and
correctness remain hard CI gates.

## Downstream Integration

PTO-SPEC and DavinciOO consume an exact NDF Git submodule commit. Their wrappers
invoke the Rust binary built with `cargo build --locked --release`; CI caches the
Cargo target directory but treats the binary as derived state.

No downstream workflow installs NDF with `pip`, imports `ndf` as a Python module,
or depends on PyYAML. Cross-repository integration uses the CLI JSON contract and
versioned files, not Rust crate internals.

## Rollback and Release Gate

Do not create `v0.1.0` and do not advance PTO-SPEC or DavinciOO pins until all
Rust parity and release gates pass on the merged commit. If the migration cannot
meet a contract, fix the Rust implementation or explicitly revise the design;
do not silently fall back to a supported dual-language architecture.

The Python prototype remains available at merge commit
`b0df8a0e9d95ed1946204606392049d2ef3133c1` for diagnosis. It is not a release,
runtime dependency, or alternate supported implementation.
