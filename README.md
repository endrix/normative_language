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

## Documentation

- [English specification](normative_language.md)
- [Chinese specification](normative_language_cn.md)
- [Format 0.1 to 0.2 migration](docs/format/migration-v0.1-to-v0.2.md)
- [PTO ASL adapter](docs/adapters/pto-asl.md)

Format 0.2, canonical IR 0.1, JSON output schema 0.1, CLI spelling, and the
plugin API are experimental. Downstream repositories consume NDF from an exact
Git commit recorded in `ndf.lock`; CI must not fetch an unpinned branch.
