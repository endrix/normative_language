# Changelog

All notable changes to NDF are documented here.

## Unreleased

### Added

- `ndf diagram` emits one document as a diagram graph document (`--format
  graph` for a diagram consumer, `--format json` for the envelope, `--group
  refinement|domain` for one container per layer or domain), with header
  badges for modality, layer and a non-active status and a facts list per
  clause; the `ndf-sidecar` binary answers a diagram platform's
  `getCapabilities` and `exportGraph` operations over the sidecar protocol.
  `--workflow NAME` names the unit (the project, a document by id or title, a
  clause by id) and `--view TYPE` draws its trace along one edge type through
  the whole project; `ndf diagram ndf.yaml` is the project overview, one node
  per document with the edges between documents added up. Every document
  carries the project outline (`hierarchy`) and its views (`meta.views`); a
  clause outside the document drills into the document that holds it, a
  target in a locked graph dependency is drawn with its source in that
  checkout. A project without dependencies is drawn without an `ndf.lock`.
  `ndf-sidecar` edits documents: `createNode`, `renameNode`, `deleteNode`,
  `connect`, `deleteEdge` and `updateNodeParameter`, each computed on the text,
  re-parsed before it is written and refused when the result parses worse,
  with `expectedRevision` conflicts; its capabilities list the edge types as
  `edgeRoles`. `ndf_compiler::edit` holds the text operations and
  `markdown::parse_markdown_text` parses a document from text.
- Standalone Rust 1.94.0 knowledge compiler and versioned JSON CLI.
- Canonical graph IR 0.1 with project-scoped `ndf://` identities.
- Strict Markdown, PTO ASL, repository, test, and evidence adapters.
- Deterministic SQLite schema v1 with semantic build hashes and provenance.
- JSON workflows for build, check, show, trace, coverage, dependency reports,
  semantic diff, export, ID allocation, and safe fixes.
- Exact dependency locking and no-regression diagnostic baselines.
- Read-only graph dependencies plus `conforms-to` and expected-absent `tracks`
  relationships for cross-project contracts.
- Stable PTO instruction identities with indexed encoding, semantics, legality,
  fault, completion, architectural-state, and memory-ordering facets.
- Typed PTO ASL unit provenance, synthetic decoder, and architecture-state
  nodes with complete-project dependency, owner, member, duplicate, and cycle
  validation through diagnostics `NDF-ASL-006` to `NDF-ASL-009`.
- Indexed attribute queries through `ndf query`, including
  `attributes.pto_entity=state`.
- Deterministic PTO release impact analysis and the `ndf impact pto-release`
  command, including complete conformance/watch inventories and transitive
  consumer impact, state-semantic breakage, state-owner moves, and state
  closure expansion review.
- Cross-project diagnostics `NDF-XREF-001` through `NDF-XREF-004` and PTO
  instruction identity collision diagnostic `NDF-ASL-005`.

### Changed

- Replaced the pre-release Python prototype with a Rust-only workspace; Python
  remains available only in Git history as migration evidence.
- NDF format 0.2 separates `kind`, `modality`, `refinement`, `domain`, and
  `status`.
- SQLite is defined as derived, disposable state rather than an authored
  authority.
- Dependency graphs remain separate namespaces and are never merged into a
  consumer index; every graph dependency is loaded only from an exact, clean
  Git revision.

### Compatibility

- DavinciOO format 0.1 metadata is accepted through a diagnostic migration
  adapter.
- PTO-SPEC ASL retains architecture authority and is normalized through the
  PTO adapter.
- NDF has no supported Python package, import API, or PyO3 compatibility layer.
- Format, IR, JSON output, CLI, and plugin surfaces remain experimental.
- This release does not change format 0.2, canonical IR 0.1, JSON envelope
  schema 0.1, SQLite schema v1, or the independently versioned plugin API.
