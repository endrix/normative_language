# Changelog

All notable changes to NDF are documented here.

## Unreleased

### Added

- Canonical graph IR 0.1 with project-scoped `ndf://` identities.
- Strict Markdown, PTO ASL, repository, test, and evidence adapters.
- Deterministic SQLite schema v1 with semantic build hashes and provenance.
- JSON workflows for build, check, show, trace, coverage, dependency reports,
  semantic diff, export, ID allocation, and safe fixes.
- Exact dependency locking and no-regression diagnostic baselines.

### Changed

- NDF format 0.2 separates `kind`, `modality`, `refinement`, `domain`, and
  `status`.
- SQLite is defined as derived, disposable state rather than an authored
  authority.

### Compatibility

- DavinciOO format 0.1 metadata is accepted through a diagnostic migration
  adapter.
- PTO-SPEC ASL retains architecture authority and is normalized through the
  PTO adapter.
- Format, IR, JSON output, CLI, and plugin surfaces remain experimental.
