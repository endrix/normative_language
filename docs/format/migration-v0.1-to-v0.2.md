# Migrating NDF Format 0.1 to 0.2

Format 0.2 separates concepts that format 0.1 overloaded, adds project-scoped
URIs and lock files, and defines SQLite as a disposable query index. Authored
Markdown, ASL, source, tests, and evidence remain the only writable truth.

## DavinciOO Markdown

Before:

```markdown
## Issue contract {#PIPE-ISSUE-001}
<!-- ndf: kind=req level=must layer=L1 status=stable -->
```

After:

```markdown
---
doc_id: DOC-DAVINCIOO-PIPELINE
status: active
authority: normative
owner: core
---

## Issue contract {#PIPE-ISSUE-001}
<!-- ndf: kind=requirement modality=must refinement=L1 domain=core status=active -->
```

The compatibility adapter maps the old fields and emits `NDF-MIG-001`. Teams
should remove each migration diagnostic by rewriting the source explicitly;
the adapter is not a second normative dialect.

## PTO-SPEC ASL

PTO's authored syntax remains ASL-owned:

```asl
// NDF-BEGIN: PTO-TILE-CAPACITY
// ndf: kind=contract level=L1 layer=tile status=accepted
// Tile capacity is defined per selected PE.
// NDF-END: PTO-TILE-CAPACITY
```

The PTO adapter normalizes `kind=contract` to canonical
`kind=requirement`, `level=L1` to `refinement=L1`, `layer=tile` to
`domain=tile`, and `status=accepted` to `status=active`. This mapping does not
move architectural authority out of PTO-SPEC ASL.

## Project Identity and Dependencies

Add `ndf.yaml` with a lowercase project ID, roots, ID prefixes, domains, and
named dependency paths. Add `ndf.lock` with the exact 40-hex Git commit for
every dependency. Local IDs resolve inside the declaring project; use a full
URI such as `ndf://pto-spec/PTO-TILE-CAPACITY` across projects.

Run:

```bash
cargo build --locked --release -p ndf-cli
target/release/ndf build --root . --output .ndf-cache/graph.sqlite --format json
target/release/ndf check --root . --format json
```

When NDF is checked out as a submodule, use its `scripts/ndf` wrapper or invoke
the built binary through a subprocess and parse only the versioned JSON
envelope. Do not install or import a Python NDF package.

Do not commit the SQLite database. Commit authored changes, manifests, locks,
and reviewed evidence instead.

## Compatibility and Maturity

Format version, IR version, JSON output schema, CLI spelling, and plugin API
are independent. All are experimental in NDF 0.1.0. Pin an exact NDF commit,
record semantic-diff evidence when advancing it, and retain a previous checker
until repository-wide parity is demonstrated.
