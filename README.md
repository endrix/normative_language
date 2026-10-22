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

### Cross-project conformance

Declare an external graph only when document relationships must resolve into
that dependency:

```yaml
dependencies:
  pto-spec:
    path: deps/pto-spec
    graph: true
```

The matching `ndf.lock` entry must name the dependency project and its full
40-character Git commit. NDF verifies that the checkout is exact and clean,
loads its graph read-only, and keeps its nodes out of the consumer index.

Consumer Markdown can then use a stable external URI:

```markdown
<!-- ndf: kind=requirement modality=must refinement=L1 status=active conforms-to=ndf://pto-spec/PTO-INST-TILE-TLOAD -->
```

Use `tracks=ndf://... expected=absent` when a consumer intentionally watches
for an upstream identity that does not exist yet. `NDF-XREF-001` reports an
invalid watch contract, `NDF-XREF-002` a missing conformance target,
`NDF-XREF-003` an undeclared graph namespace, and `NDF-XREF-004` a watched
identity that has appeared.

### Diagrams

`ndf diagram` emits one document as the graph document a diagram consumer
reads, with the document's clauses as nodes and its typed edges as
connections:

```bash
ndf diagram docs/pipeline.md --format graph
```

`--group refinement` (or `domain`) puts the document's clauses into one
container per layer or domain; the default, `none`, draws them flat. Each
clause carries badges for its modality, its layer and a status other than
active, and a `facts` list with its canonical fields and body for a panel to
show. `--format graph` prints the bare document for a consumer such as the
[Dialogram](https://github.com/endrix) `ndf-ide` shell, which parses stdout as
is; `--format json` wraps it in the usual envelope. The command finds the
project from the file, parses and validates it the way `check` does, and exits
with status 2 only when the file is in no NDF project at all. A project that
fails to load yields a partial document carrying the reason. `ndf-sidecar`,
built beside `ndf`, speaks the sidecar protocol a diagram consumer expects:
`getCapabilities` and `exportGraph`, and no mutation.

## Documentation

- [English specification](normative_language.md)
- [Chinese specification](normative_language_cn.md)
- [Format 0.1 to 0.2 migration](docs/format/migration-v0.1-to-v0.2.md)
- [PTO ASL adapter](docs/adapters/pto-asl.md)

Format 0.2, canonical IR 0.1, JSON output schema 0.1, CLI spelling, and the
plugin API are experimental. Downstream repositories consume NDF from an exact
Git commit recorded in `ndf.lock`; CI must not fetch an unpinned branch.
This cross-project release keeps format 0.2, canonical IR 0.1, JSON envelope
schema 0.1, SQLite schema v1, and the plugin API version unchanged.
