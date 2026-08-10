# NDF Knowledge Compiler: Cross-Repository Refactoring Design

**Status:** Proposed for review
**Date:** 2026-08-10
**Repositories:** `PTO-ISA/normative_language`, `PTO-ISA/pto-spec`,
`hengliao1972/DavinciOO`

## 1. Summary

NDF will evolve from a Markdown convention plus a proposed command list into a
repository-independent knowledge compiler.

Markdown and Git remain the only writable source of truth. The compiler parses
normative clauses, ordinary documents, external specifications, source files,
tests, decisions, open questions, and evidence into a canonical knowledge IR.
It validates that IR and materializes a disposable SQLite index for querying,
traceability, semantic diff, coverage, and agent retrieval.

The work is intentionally split across three repositories:

- `PTO-ISA/normative_language` owns the NDF specification, canonical IR,
  compiler, generic rules, CLI, and compatibility SDK.
- `PTO-ISA/pto-spec` owns PTO architectural truth in ASL and supplies an
  ASL-NDF adapter corpus. It must not grow a competing general NDF engine.
- `hengliao1972/DavinciOO` is the first production consumer. It owns the
  DavinciOO project manifest, Markdown NDF corpus, document hierarchy,
  project-specific policy, and end-to-end conformance links to PTO-SPEC.

NDF format, IR, CLI, and plugin API mature independently. Real usage in
PTO-SPEC and DavinciOO drives revisions; early command spellings are not treated
as permanent APIs.

## 2. Goals

1. Make a large documentation corpus queryable and mechanically maintainable
   without making a database the authoring surface.
2. Give normative clauses, documents, decisions, tests, and evidence stable
   identities that survive file moves.
3. Support a cross-repository graph with pinned, reproducible dependencies.
4. Enforce the indexing rules for humans and agents through deterministic CI,
   not through prompt discipline alone.
5. Preserve both existing NDF dialects during migration:
   DavinciOO Markdown clauses and PTO-SPEC ASL comment regions.
6. Allow the NDF design and tool to evolve together while preserving explicit
   migrations and compatibility evidence.
7. Scale to thousands of documents and at least hundreds of thousands of graph
   nodes without changing the source format.

## 3. Non-goals

- SQLite is not an editing API and is never committed as normative content.
- The first release does not include a web application, background daemon,
  hosted service, or collaborative database editor.
- The compiler does not decide requirement meaning, authority, supersession,
  or conflict resolution.
- The compiler does not replace ASL, source control, code review, test runners,
  or physical-design databases.
- The first release does not promise stable CLI spelling or a stable plugin
  ABI.

## 4. Authority Model

| Question | Authority |
| --- | --- |
| NDF format and canonical field semantics | `PTO-ISA/normative_language` specification |
| PTO programmer-visible architecture | pinned `PTO-ISA/pto-spec` ASL |
| DavinciOO microarchitecture contracts | DavinciOO NDF clauses |
| DavinciOO explanatory architecture | DavinciOO architecture documents |
| Executable implementation | repository source trees |
| Verification result | referenced test/evidence artifact at a recorded revision |
| Query/index result | derived SQLite index, reproducible from the sources above |

An index may report authority; it cannot create authority.

## 5. Architectural Approach

NDF is implemented as a compiler pipeline:

```text
Git-tracked sources
    |
    +-- Markdown/front matter/clauses
    +-- PTO ASL NDF regions
    +-- source and test adapters
    +-- evidence manifests
    +-- pinned external projects
    v
Parser adapters
    v
Canonical Knowledge IR
    v
Validation and project rules
    v
Disposable SQLite index
    v
check / show / query / trace / report / diff / export
```

The canonical IR, not a parser-specific record and not the SQLite schema, is
the compatibility boundary between authoring formats and query features.

### 5.1 Proposed package boundaries

| Component | Responsibility |
| --- | --- |
| `ndf-core` | Canonical node, edge, identity, source-location, diagnostic, and baseline types |
| `ndf-parser` | Markdown, front matter, clause, reference, and manifest parsing |
| `ndf-adapters` | PTO ASL, generic source, test, evidence, and external-project adapters |
| `ndf-rules` | Generic validation rules and project-rule extension points |
| `ndf-index` | Deterministic SQLite materialization and schema migrations |
| `ndf-query` | Node lookup, graph traversal, coverage, status, and semantic diff |
| `ndf-cli` | Human and agent command surface with JSON output |

The compiler is implemented in Rust. The initial Cargo workspace groups these
boundaries into `ndf-core`, `ndf-compiler`, and `ndf-cli` rather than creating a
crate for every row in the table. The detailed migration and release contract is
defined in
[`2026-08-10-ndf-rust-compiler-migration-design.md`](../superpowers/specs/2026-08-10-ndf-rust-compiler-migration-design.md).

## 6. Canonical Data Model

### 6.1 Identity

Every project declares a stable project ID. Canonical identities are URIs:

```text
ndf://davincioo/VEC-ISSUE-001
ndf://pto-spec/PTO-INST-TILE-TLOAD
ndf://pycircuit/PYC-PIPE-001
```

Inside one project, `[[VEC-ISSUE-001]]` is valid shorthand. Cross-project
references must name the project explicitly.

Use a mixed identity policy:

- Normative clauses, important documents, decisions, questions, verification
  specifications, and curated evidence use explicit stable IDs.
- Ordinary source files, generated files, and ephemeral run artifacts use
  derived identities based on project, path, revision, and content identity.
- Moving an explicitly identified object must not change its identity.
- Derived identities are not suitable targets for long-lived normative edges
  unless promoted to an explicit ID.

### 6.2 Canonical fields

Existing dialects overload `kind`, `level`, and `layer`. The canonical IR uses
separate dimensions:

```yaml
kind: requirement
modality: must
refinement: L1
domain: tile
status: active
```

Initial canonical enums are:

- `kind`: `document`, `requirement`, `definition`, `architecture`,
  `constraint`, `option`, `verification`, `decision`, `question`, `model`,
  `source`, `test`, `evidence`, `information`.
- `modality`: `must`, `should`, `may`, `tbd`, or absent where inapplicable.
- `refinement`: project-defined ladder, normally `L0` through `L3`.
- `domain`: a project-owned namespace, not a global enum.
- `status`: `draft`, `active`, `deprecated`, `superseded`, `retired`, plus
  kind-specific states such as question `open` and `resolved`.

Adapters map old fields into these canonical dimensions and emit migration
diagnostics. They do not silently guess ambiguous mappings.

### 6.3 Documents and clauses

Files use YAML front matter for file-level identity and lifecycle:

```yaml
---
doc_id: DOC-DAVINCIOO-VECTOR-ARCH
status: active
authority: informative
owner: vector
---
```

Heading-anchored clauses retain local metadata immediately after the heading:

```markdown
## Vector issue contract {#VEC-ISSUE-001}
<!-- ndf: kind=requirement modality=must refinement=L1 status=active -->
```

The authored Markdown remains readable without the tool.

### 6.4 Graph edges

The core edge vocabulary starts with:

- `refines`
- `depends-on`
- `conflicts-with`
- `verifies`
- `derived-from`
- `implements`
- `affects`
- `blocks`
- `supersedes`
- `references`
- `evidenced-by`

Projects may register additional namespaced edge types. Generic queries must
still be able to traverse them as edges without loading the project plugin.

### 6.5 SQLite materialization

The physical schema is explicitly non-normative. It will initially contain
tables equivalent to:

- projects and pinned project revisions
- source files and source locations
- nodes and kind-specific attributes
- aliases and external identities
- typed edges
- baselines
- diagnostics and waivers
- content hashes and index-build provenance

Deleting the database and rebuilding it from the same locked sources must
produce semantically identical rows and query results.

## 7. Dependency and Baseline Model

External projects are declared in `ndf.yaml` and resolved into `ndf.lock`:

```yaml
project: davincioo
dependencies:
  pto-spec:
    uri: https://github.com/PTO-ISA/pto-spec.git
    revision: 0123456789abcdef0123456789abcdef01234567
    path: isa
```

The hexadecimal revision above is illustrative. A committed lock records the
actual 40-character commit selected by the DavinciOO `isa` gitlink.

Formal baselines only index exact commits or signed/tagged releases. A local
override may point to a developer checkout, but an override makes the build
non-release and cannot update the committed lock.

Every semantic diff, verification campaign, PPA summary, or published view
records:

- source project revisions
- NDF format version
- canonical IR version
- rule-set versions
- index build identity

## 8. Evolution and Compatibility

The project exposes independent maturity markers:

```yaml
format_version: 0.2
ir_version: 0.1
cli_stability: experimental
plugin_api: experimental
```

Maturity progresses independently:

```text
experimental -> provisional -> stable
```

- Experimental surfaces may change with release notes and migration commands.
- Provisional surfaces require deprecation warnings and at least one supported
  migration path.
- Stable surfaces follow semantic versioning and a documented compatibility
  window.

The CLI is task-oriented but its early spelling is not normative. Initial
capabilities are build, check, show, query, trace, report, diff, ID allocation,
and safe mechanical repair. All agent-facing commands support versioned JSON
output; agents must not parse terminal prose.

## 9. Agent Enforcement Contract

Compliance is enforced by tools and CI rather than relying on instructions.

1. Agents and humans must not edit SQLite indexes.
2. Explicit IDs are allocated through the NDF ID service/command.
3. File moves preserve explicit IDs.
4. Changed documents run changed-scope validation before completion.
5. CI rebuilds the full index and runs the complete rule set.
6. Duplicate IDs, dangling mandatory edges, illegal lifecycle transitions,
   non-reproducible indexes, and unauthorized database artifacts fail CI.
7. Safe repair may format metadata, regenerate derived navigation, or apply an
   explicit schema migration. It may not decide semantic content.
8. Agent guidance in `AGENTS.md` or equivalent is generated or checked against
   the project manifest so command drift is detected.

For an existing corpus with known violations, the first hard gate compares
against a checked-in diagnostic baseline: no new diagnostic and no severity
increase is allowed. Baseline entries require owners and expiry milestones.
This is a hard no-regression gate, not a warning-only rollout.

## 10. Repository Responsibilities

### 10.1 `PTO-ISA/normative_language`

Owns:

- NDF specification in English and Chinese
- canonical JSON Schema for manifests, front matter, IR export, and diagnostics
- compiler and SQLite indexer
- generic CLI and JSON contracts
- compatibility adapters and adapter SDK
- format/IR migrations
- conformance fixture suite
- release notes and compatibility matrix

It must include small, redistributed fixtures representing both PTO ASL NDF and
DavinciOO Markdown NDF. Full external repositories remain end-to-end consumers,
not vendored test data.

The supported compiler is a standalone Rust binary. The merged Python prototype
is a migration oracle only and is removed before the first `v0.1.0` release.

### 10.2 `PTO-ISA/pto-spec`

Owns:

- architecture authority in ASL
- PTO-specific project manifest and namespaces
- mapping from current ASL NDF fields to canonical fields
- instruction and clause identity export
- PTO adapter integration tests
- compatibility test proving the generic compiler preserves existing
  `scripts/ndf.py` validation behavior before that code is retired

The current ASL parser is first wrapped as a golden oracle. Once the generic
adapter matches its accepted and rejected fixture corpus, PTO-SPEC replaces the
duplicate parser with the released NDF engine.

### 10.3 `hengliao1972/DavinciOO`

Owns:

- DavinciOO project manifest, namespace, domains, owners, and rule plugin config
- Markdown NDF corpus and stable IDs
- document-level front matter for the broader docs corpus
- cross-project references into the pinned `isa/` PTO-SPEC checkout
- source/test/evidence mappings
- generated navigation and repository CI integration
- diagnostic-debt baseline during migration

DavinciOO does not fork generic parsing rules. A rule that cannot be expressed
by manifest configuration is proposed upstream as a generic extension or
implemented as an explicitly namespaced DavinciOO rule.

## 11. Error Handling and Diagnostics

Every diagnostic has a stable code, severity, primary source span, related
spans, project, rule version, and machine-readable repair classification.

Example classes:

- syntax and metadata parse errors
- identity collisions
- unresolved local or external references
- authority inversion
- invalid state transition
- missing verification coverage
- missing owner or expired waiver
- stale generated navigation
- non-deterministic index output
- incompatible format or dependency version

The compiler accumulates independent errors rather than stopping at the first
one. A partial index may be created for diagnostics, but it is marked invalid
and cannot be used as a release baseline.

## 12. Verification Strategy

### 12.1 `normative_language` tests

- parser unit tests for front matter, clauses, references, and malformed input
- canonical IR serialization round trips
- SQLite rebuild determinism
- query and graph traversal tests
- migration tests for every supported format transition
- stable diagnostic-code tests
- golden adapters for PTO ASL and DavinciOO Markdown
- scale test with a generated corpus of at least 100,000 nodes and 1,000,000
  edges; exact performance gates remain provisional until real measurements

### 12.2 PTO-SPEC tests

- replay every current `test_ndf.py` accept/reject case through both old and new
  implementations
- compare clause identities and references exported from the complete ASL tree
- verify no change to ASL authority, build, or release closure
- verify a pinned generic NDF release is sufficient; do not fetch unpinned NDF
  source in CI

### 12.3 DavinciOO tests

- full-corpus duplicate ID and dangling edge validation
- PTO cross-project reference resolution against the submodule pin
- current/historical document lifecycle checks
- NDF requirement-to-test/evidence coverage reports
- generated navigation freshness
- source, test, and evidence path existence
- clean rebuild equality for the SQLite index
- changed-scope agent gate plus full CI gate

### 12.4 Cross-repository compatibility test

A release candidate of the NDF engine is tested against pinned PTO-SPEC and
DavinciOO revisions. The compatibility result records all three commits and is
required before an NDF release is promoted from experimental to provisional.

## 13. Cross-Repository Refactoring Sequence

### Stage 0: Freeze observed behavior

- Capture the current normative-language examples as specification fixtures.
- Export PTO-SPEC's existing NDF accepted/rejected cases as a golden corpus.
- Export DavinciOO's current node, edge, decision, and question census without
  changing authored files.
- Record all current ambiguities between the two dialects.

Exit condition: each repository has a reproducible pre-migration snapshot and
no migration step needs to infer the old behavior from memory.

### Stage 1: Establish the generic compiler core

- Revise the NDF specification to define derived indexes, canonical fields,
  namespaced identities, document front matter, pinned dependencies, maturity,
  and agent enforcement.
- Add project and lock schemas.
- Implement Markdown parsing, canonical IR, diagnostics, deterministic SQLite
  indexing, and JSON output.
- Keep command names experimental.

Exit condition: the small upstream fixture suite builds, checks, queries, and
rebuilds deterministically.

### Stage 2: Add PTO-SPEC compatibility

- Implement the ASL-region adapter.
- Run old and new validators side by side.
- Resolve field mappings explicitly and add migration diagnostics.
- Release an experimental NDF version only after parity is reached.
- Pin that version in PTO-SPEC and retire duplicate general validation logic
  while keeping PTO-specific rules local.

Exit condition: PTO-SPEC has no NDF behavior regression and consumes a released,
pinned generic engine.

### Stage 3: Index DavinciOO without restructuring

- Add DavinciOO `ndf.yaml` and `ndf.lock`.
- Index the current NDF and broader docs in place.
- Add explicit IDs only where long-lived relationships require them.
- Create the diagnostic baseline with owners.
- Connect cross-project PTO references through the existing `isa/` pin.
- Add agent and CI hard no-regression gates.

Exit condition: the entire current repository is queryable, every new violation
is blocked, and no document move has occurred yet.

### Stage 4: Refactor the DavinciOO hierarchy using indexed moves

- Promote the DavinciOO NDF to the single microarchitecture authority.
- Consolidate canonical navigation into one generated root view.
- Move explanatory architecture into domain directories.
- Create first-class implementation, verification, optimization, and physical
  design areas.
- Move historical revisions into explicit historical storage.
- Use stable IDs and semantic diff to prove moves did not change meaning.

Exit condition: current, proposed, generated, and historical content are
unambiguous; all incoming references resolve after the moves.

### Stage 5: Close migration debt and stabilize interfaces

- Remove expired diagnostic-baseline entries.
- Promote proven format and IR surfaces to provisional.
- Freeze the first machine-output compatibility contract.
- Publish the cross-repository trace and coverage reports.
- Decide, from observed usage, which CLI spellings merit stability.

Exit condition: zero unowned migration debt, reproducible three-repository
compatibility evidence, and a documented next compatibility window.

## 14. Rollback and Failure Containment

- Each repository migrates in its own PR; cross-repository pins advance only
  after the upstream PR is merged and released.
- DavinciOO does not depend on an unreleased NDF branch in its default branch.
- PTO-SPEC keeps the old validator available for one compatibility window after
  the new adapter is enabled.
- The DavinciOO hierarchy is moved only after the in-place index is green, so a
  failed reorganization can be reverted without losing identity history.
- SQLite schema changes never block rollback because indexes are disposable;
  source-format changes require explicit reverse or forward migration guidance.

## 15. Main Risks

| Risk | Mitigation |
| --- | --- |
| Early DavinciOO needs distort the generic format | Project rules are namespaced; generic changes require upstream fixtures from more than one domain |
| PTO ASL and Markdown dialects remain semantically inconsistent | Canonical dimensions plus explicit adapter diagnostics; no silent mapping |
| Agents bypass conventions | CI rebuild and hard no-regression gate; database edits rejected |
| Command churn breaks automation | Versioned JSON capability contracts and deprecated aliases during provisional maturity |
| Cross-repository updates deadlock | Strict upstream release and downstream pin order; no circular repository dependency |
| Index becomes a hidden second truth | Disposable untracked database, deterministic rebuild check, no write API |
| Existing documentation debt blocks adoption | Checked-in owned baseline that blocks all new debt and expires by milestone |

## 16. Acceptance Criteria for the Design

The subsequent implementation plan must demonstrate that:

1. One clean checkout can reconstruct the same canonical graph and SQLite index.
2. An agent cannot merge a duplicate ID, dangling required edge, or direct
   database edit.
3. PTO-SPEC keeps ASL authority and preserves its existing NDF validation
   behavior through the adapter transition.
4. DavinciOO can resolve a Markdown NDF clause to PTO ASL, implementation
   source, verification criterion, and evidence where those links exist.
5. Moving a document does not change explicit identities or semantic query
   results.
6. Existing debt is visible and owned; new debt fails CI immediately.
7. Format, IR, CLI, and plugin maturity are independently versioned.
8. The three repositories can release and roll back independently without a
   circular dependency.
