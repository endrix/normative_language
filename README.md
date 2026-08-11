# NDF

NDF is an experimental knowledge compiler for normative engineering documents.
Markdown, ASL, source, test, and evidence files remain Git-tracked sources of
truth; generated indexes are disposable query artifacts.

## Development

NDF requires Rust 1.94.0. The repository is a locked Cargo workspace:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --locked
cargo build --workspace --release --locked
```

Print the independently versioned public surfaces with:

```bash
scripts/ndf --machine-version
```

`ndf-core` owns canonical identities, graph types, and public version constants.
`ndf-compiler` owns adapters, validation, SQLite materialization, and graph
workflows. `ndf-cli` is the only supported automation interface. NDF ships no
Python package or Python bindings; downstream tools invoke the versioned JSON
CLI from an exact Git pin.

### PTO release impact

Compare two immutable PTO indexes against one consumer index with:

```bash
ndf impact pto-release \
  --before pto-before.sqlite \
  --after pto-after.sqlite \
  --consumer-index davincioo.sqlite \
  --format json
```

The two PTO indexes are authoritative release snapshots. The consumer index is
the authoritative inventory of `conforms-to` relationships and
`expected=absent` `tracks` watches. The command only reads these three indexes;
it does not edit sources, indexes, lock files, or dependency checkouts.

The result is `compatible` when changes do not affect a consumer contract,
`review-required` for moved or superseded targets and newly present watched
identities, and `breaking` when a conformance target is removed or semantically
modified. The report includes direct and transitively related consumers in a
deterministic order so CI and release tooling can compare its JSON byte for
byte.

## Documentation

- [English specification](normative_language.md)
- [Chinese specification](normative_language_cn.md)
- [Format 0.1 to 0.2 migration](docs/format/migration-v0.1-to-v0.2.md)
- [PTO ASL adapter](docs/adapters/pto-asl.md)

Format 0.2, canonical IR 0.1, JSON output schema 0.1, CLI spelling, and the
plugin API are experimental. Downstream repositories consume NDF from an exact
Git commit recorded in `ndf.lock`; CI must not fetch an unpinned branch.
