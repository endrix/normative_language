# NDF Three-Repository Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the NDF knowledge compiler in `PTO-ISA/normative_language`, migrate `PTO-ISA/pto-spec` to its ASL adapter without changing architecture authority, then use the indexed graph to reorganize and hard-gate the DavinciOO documentation corpus.

**Architecture:** Markdown, ASL, source, test, and evidence files remain Git-tracked sources of truth. Parser adapters normalize them into a versioned canonical IR, generic and project rules validate that IR, and a deterministic SQLite database provides disposable query indexes. The dependency direction is strictly `DavinciOO -> normative_language`, `DavinciOO -> pto-spec`, and `pto-spec -> normative_language`; no upstream repository imports DavinciOO.

**Tech Stack:** Python 3.11+, `dataclasses`, `enum.StrEnum`, `argparse`, `sqlite3`, `json`, `pathlib`, PyYAML 6.x, `unittest`, Markdown, Git, Git submodules, GitHub Actions.

## Global Constraints

- Markdown and Git are the only writable normative sources; SQLite files are disposable and untracked.
- Use canonical URIs such as `ndf://davincioo/PIPE-STG-001`; short IDs resolve only inside their declaring project.
- Keep `kind`, `modality`, `refinement`, `domain`, and `status` as separate canonical dimensions.
- Keep PTO programmer-visible behavior authoritative in PTO-SPEC ASL.
- Keep DavinciOO microarchitecture authority in DavinciOO NDF clauses.
- Do not silently infer ambiguous legacy-field mappings; emit stable diagnostics.
- Install downstream NDF code from an exact Git submodule commit; never fetch an unpinned NDF branch in CI.
- All agent-facing commands provide versioned JSON output; automation must not parse terminal prose.
- Safe fix operations may change formatting or generated navigation only; they may not decide normative semantics.
- Existing DavinciOO debt enters an owned, expiring baseline; every new or worsened diagnostic fails CI immediately.
- Keep English and Chinese NDF specifications synchronized in the same commit.
- Execute the plan inline, in repository-isolated worktrees, with one repository and one task active at a time.

---

## Repository and PR Sequence

| Order | Repository | Branch / PR purpose | Consumed by |
| --- | --- | --- | --- |
| 1 | `PTO-ISA/normative_language` | Canonical IR, Markdown parser, checks, SQLite index, CLI | PTO-SPEC and DavinciOO |
| 2 | `PTO-ISA/normative_language` | PTO ASL adapter and experimental `v0.1.0` release | PTO-SPEC |
| 3 | `PTO-ISA/pto-spec` | Pin NDF, prove old/new parity, then switch `check-ndf` | DavinciOO through `isa/` |
| 4 | `PTO-ISA/normative_language` | PTO parity fixes/evidence and experimental `v0.1.1` release | DavinciOO |
| 5 | `hengliao1972/DavinciOO` | Pin NDF, index current tree in place, add hard no-regression gate | hierarchy refactor |
| 6 | `hengliao1972/DavinciOO` | Move NDF and architecture docs using stable identities | final corpus |
| 7 | `PTO-ISA/normative_language` | Cross-repository compatibility report; promote proven surfaces to provisional | next release train |

Do not start a downstream PR until the exact upstream commit or tag it consumes
exists. Each downstream PR records the consumed commit in its lock and PR body.

## Rollback Checkpoints

- Before PTO switches validators, `scripts/ndf_legacy.py` remains executable and
  the parity test can restore it as the wrapper target in one revert.
- Before DavinciOO moves any file, it stores a semantic export and index hash;
  the move PR can be reverted without changing IDs or the earlier indexing PR.
- NDF SQLite schema changes require no source rollback because indexes rebuild;
  source-format changes require an explicit migration test and release note.
- A downstream pin advances only in a dedicated commit, so reverting that
  commit restores the last compatible NDF/PTO tuple without reverting content.

---

### Task 1: Scaffold the NDF package and freeze public version surfaces

**Repository:** `PTO-ISA/normative_language`

**Files:**
- Create: `pyproject.toml`
- Create: `requirements-lock.txt`
- Create: `src/ndf/__init__.py`
- Create: `src/ndf/__main__.py`
- Create: `src/ndf/version.py`
- Create: `src/ndf/cli.py`
- Create: `tests/test_cli.py`
- Create: `.github/workflows/ci.yml`
- Create: `.gitignore`
- Create: `README.md`

**Interfaces:**
- Produces: `ndf.cli.main(argv: Sequence[str] | None = None) -> int`
- Produces: `python -m ndf ...` as the installation-independent CLI entrypoint.
- Produces: `ndf --machine-version` JSON with `tool_version`, `format_version`, `ir_version`, `cli_stability`, and `plugin_api`.

- [ ] **Step 1: Write the failing CLI contract test**

```python
import json
import unittest
from contextlib import redirect_stdout
from io import StringIO

from ndf.cli import main


class CliVersionTest(unittest.TestCase):
    def test_machine_version_has_independent_surfaces(self) -> None:
        output = StringIO()
        with redirect_stdout(output):
            status = main(["--machine-version"])
        self.assertEqual(status, 0)
        self.assertEqual(
            json.loads(output.getvalue()),
            {
                "cli_stability": "experimental",
                "format_version": "0.2",
                "ir_version": "0.1",
                "plugin_api": "experimental",
                "tool_version": "0.1.0",
            },
        )
```

- [ ] **Step 2: Run the test and confirm the package is absent**

Run: `python3 -m unittest tests.test_cli -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'ndf'`.

- [ ] **Step 3: Add the minimal package and CLI**

Use setuptools with package root `src`, require Python `>=3.11`, and declare
`PyYAML>=6.0,<7` as the only runtime dependency. Add:

```toml
[project.scripts]
ndf = "ndf.cli:entrypoint"
```

Implement `version.py` constants and `main()` with `argparse`. `entrypoint()`
raises `SystemExit(main())` so tests can call `main()` directly;
`src/ndf/__main__.py` imports and invokes `entrypoint()`.
Pin `PyYAML==6.0.2` in `requirements-lock.txt` for CI and downstream
reproducibility.

- [ ] **Step 4: Add CI and ignore generated state**

The workflow uses Python 3.11 and 3.12 and runs:

```bash
python -m pip install -r requirements-lock.txt
python -m pip install -e . --no-deps
python -m unittest discover -s tests -p 'test_*.py'
git diff --check
```

Ignore `.venv/`, `build/`, `dist/`, `*.egg-info/`, `*.sqlite`, and
`.ndf-cache/`.

- [ ] **Step 5: Verify and commit**

Run: `python3 -m pip install -r requirements-lock.txt && python3 -m pip install -e . --no-deps && python3 -m unittest discover -s tests -p 'test_*.py' && git diff --check`
Expected: PASS.

```bash
git add pyproject.toml requirements-lock.txt src tests .github .gitignore README.md
git commit -m "feat: scaffold the NDF knowledge compiler"
```

---

### Task 2: Define canonical identities, nodes, edges, and diagnostics

**Repository:** `PTO-ISA/normative_language`

**Files:**
- Create: `src/ndf/model.py`
- Create: `src/ndf/identity.py`
- Create: `schemas/ndf-ir.schema.json`
- Create: `schemas/ndf-diagnostic.schema.json`
- Create: `tests/test_model.py`
- Create: `tests/test_identity.py`

**Interfaces:**
- Produces: `canonical_uri(project_id: str, local_id: str) -> str`
- Produces: `parse_uri(value: str, current_project: str | None = None) -> NodeId`
- Produces immutable `SourceSpan`, `Node`, `Edge`, `Diagnostic`, `Graph`, and `ParseResult` dataclasses.
- `NodeId` is an immutable `(project_id, local_id)` value whose string form is
  the canonical URI; `ParseResult` contains `nodes`, `edges`, and `diagnostics`.

- [ ] **Step 1: Write identity tests**

```python
class IdentityTest(unittest.TestCase):
    def test_local_id_resolves_inside_current_project(self) -> None:
        self.assertEqual(
            str(parse_uri("VEC-ISSUE-001", current_project="davincioo")),
            "ndf://davincioo/VEC-ISSUE-001",
        )

    def test_cross_project_reference_requires_uri(self) -> None:
        with self.assertRaisesRegex(ValueError, "current project is required"):
            parse_uri("VEC-ISSUE-001")
```

- [ ] **Step 2: Write model invariant tests**

Test that an `Edge` rejects an empty type, a `SourceSpan` rejects line zero,
and `Graph.add_node()` emits `NDF-ID-001` for a duplicate canonical URI.

- [ ] **Step 3: Run tests and confirm missing symbols**

Run: `python3 -m unittest tests.test_identity tests.test_model -v`
Expected: FAIL on missing `ndf.identity` and `ndf.model`.

- [ ] **Step 4: Implement canonical types**

Use `StrEnum` for `NodeKind`, `Modality`, `LifecycleStatus`, and
`DiagnosticSeverity`. Store project-owned `domain` and `refinement` as strings.
Use this diagnostic shape:

```python
@dataclass(frozen=True)
class Diagnostic:
    code: str
    severity: DiagnosticSeverity
    message: str
    primary: SourceSpan
    related: tuple[SourceSpan, ...] = ()
    repair: str | None = None
```

`Graph` provides insertion-independent `sorted_nodes()` and `sorted_edges()`.
Add JSON Schemas for the serialized graph and diagnostic envelopes. The model
tests load both schema documents as JSON and assert their required properties
match the dataclass serializers without adding a JSON-Schema runtime dependency.

- [ ] **Step 5: Verify and commit**

Run: `python3 -m unittest tests.test_identity tests.test_model -v`
Expected: PASS.

```bash
git add src/ndf/model.py src/ndf/identity.py schemas tests
git commit -m "feat: define the canonical NDF graph model"
```

---

### Task 3: Parse manifests, locks, document front matter, and Markdown clauses

**Repository:** `PTO-ISA/normative_language`

**Files:**
- Create: `src/ndf/manifest.py`
- Create: `src/ndf/parsers/__init__.py`
- Create: `src/ndf/parsers/markdown.py`
- Create: `schemas/ndf-project.schema.json`
- Create: `schemas/ndf-lock.schema.json`
- Create: `schemas/ndf-front-matter.schema.json`
- Create: `tests/fixtures/markdown/project/ndf.yaml`
- Create: `tests/fixtures/markdown/project/ndf.lock`
- Create: `tests/fixtures/markdown/project/docs/pipeline.md`
- Create: `tests/test_manifest.py`
- Create: `tests/test_markdown_parser.py`

**Interfaces:**
- Produces: `load_manifest(path: Path) -> ProjectManifest`
- Produces: `load_lock(path: Path) -> ProjectLock`
- Produces: `parse_markdown(path: Path, project: ProjectManifest) -> ParseResult`
- `ProjectManifest` contains format version, project ID, roots, prefixes,
  domains, and policies; `ProjectLock` contains exact dependency revisions.

- [ ] **Step 1: Add a complete fixture**

The fixture contains file front matter, informative prose, a fenced code block
with a fake heading, and two real clauses:

````markdown
---
doc_id: DOC-DAVINCIOO-PIPELINE
status: active
authority: normative
owner: core
---

# Pipeline

```text
## Not a clause {#FAKE-001}
```

## Issue contract {#PIPE-ISSUE-001}
<!-- ndf: kind=requirement modality=must refinement=L1 domain=core status=active -->

The core MUST issue no more than two instructions per cycle.

## Issue mechanism {#PIPE-ISSUE-010}
<!-- ndf: kind=requirement modality=must refinement=L2 domain=core status=active refines=PIPE-ISSUE-001 -->

Two issue slots MUST arbitrate oldest-ready first.
````

- [ ] **Step 2: Write parser tests**

Assert exactly three nodes, one `refines` edge, correct source spans, and no
`FAKE-001`. Assert stable diagnostics for invalid delimiters, duplicate keys,
and unsupported fields.

- [ ] **Step 3: Run tests and confirm failure**

Run: `python3 -m unittest tests.test_manifest tests.test_markdown_parser -v`
Expected: FAIL on missing parser modules.

- [ ] **Step 4: Implement strict parsing**

Use `yaml.safe_load`. Require project `format_version`, `project`, `roots`, and
`id_prefixes`. Accept only 40-hex dependency revisions in `ndf.lock`. Track
fenced blocks so headings inside them are ignored.

Map legacy DavinciOO fields and emit `NDF-MIG-001` information:

```text
kind=req       -> kind=requirement
level=must     -> modality=must
layer=L1       -> refinement=L1
status=stable  -> status=active
```

- [ ] **Step 5: Verify and commit**

Run: `python3 -m unittest tests.test_manifest tests.test_markdown_parser -v`
Expected: PASS.

```bash
git add src/ndf/parsers src/ndf/manifest.py schemas tests
git commit -m "feat: parse NDF projects and Markdown clauses"
```

---

### Task 4: Implement generic validation and diagnostic baselines

**Repository:** `PTO-ISA/normative_language`

**Files:**
- Create: `src/ndf/rules.py`
- Create: `src/ndf/plugins.py`
- Create: `src/ndf/baseline.py`
- Create: `schemas/ndf-diagnostic-baseline.schema.json`
- Create: `tests/test_rules.py`
- Create: `tests/test_baseline.py`

**Interfaces:**
- Produces: `validate_graph(graph: Graph, policy: ValidationPolicy) -> tuple[Diagnostic, ...]`
- Produces: `compare_baseline(current: Iterable[Diagnostic], baseline: DiagnosticBaseline) -> BaselineResult`
- Produces: `load_project_rules(manifest: ProjectManifest) -> tuple[Rule, ...]`
- `ValidationPolicy` is the normalized generic/project rule configuration;
  `DiagnosticBaseline` is the parsed owned-debt file; `BaselineResult` exposes
  `existing`, `new`, `worsened`, and `expired` diagnostics.

- [ ] **Step 1: Write failing rule tests**

Cover duplicate IDs, dangling mandatory edges, a `must` requirement without
`MUST`, `MUST` in information text, invalid `active -> draft`, and an explicit
node without an owner. Assert codes:

```text
NDF-ID-001 NDF-EDGE-001 NDF-MOD-001 NDF-MOD-002 NDF-LIFE-001 NDF-OWN-001
```

- [ ] **Step 2: Write baseline no-regression tests**

Accept an identical diagnostic, reject a new code, reject a severity increase,
and emit `NDF-BASE-003` when an entry lacks owner or expiry milestone.

- [ ] **Step 3: Run tests and confirm failure**

Run: `python3 -m unittest tests.test_rules tests.test_baseline -v`
Expected: FAIL on missing modules.

- [ ] **Step 4: Implement rules and fingerprinting**

Fingerprint from `(code, canonical URI, normalized path, normalized message
parameters)`, not line number. Baseline comparison never suppresses a
diagnostic; it classifies it as existing or new.

For format 0.2, project extension points are declarative, namespaced rule
configuration from `ndf.yaml`; do not load arbitrary Python entry points. Reject
unknown rule namespaces with `NDF-PLUGIN-001`, so a future plugin ABI can be
added without silently reinterpreting current manifests.

- [ ] **Step 5: Verify and commit**

Run: `python3 -m unittest tests.test_rules tests.test_baseline -v`
Expected: PASS.

```bash
git add src/ndf/rules.py src/ndf/plugins.py src/ndf/baseline.py schemas tests
git commit -m "feat: enforce NDF graph invariants"
```

---

### Task 5: Build a deterministic SQLite index

**Repository:** `PTO-ISA/normative_language`

**Files:**
- Create: `src/ndf/index.py`
- Create: `src/ndf/sql/schema-v1.sql`
- Create: `tests/test_index.py`
- Create: `tests/test_index_scale.py`

**Interfaces:**
- Produces: `build_index(graph: Graph, output: Path, provenance: BuildProvenance) -> IndexBuild`
- Produces: `open_index(path: Path) -> NdfIndex`
- `NdfIndex` provides `get_node(uri)`, `out_edges(uri, edge_type=None)`, and
  `in_edges(uri, edge_type=None)`.
- `BuildProvenance` contains project commits and NDF versions; `IndexBuild`
  contains output path, semantic hash, node/edge counts, and diagnostics.

- [ ] **Step 1: Write the determinism test**

Build indexes from the same graph with reversed insertion order. Query logical
tables by primary key and assert identical rows and identical
`IndexBuild.semantic_hash` values.

- [ ] **Step 2: Write immutability and provenance tests**

Assert atomic replacement, refusal to write inside a normative root, enabled
foreign keys, and stored project commits, format version, IR version, and tool
version.

- [ ] **Step 3: Run tests and confirm failure**

Run: `python3 -m unittest tests.test_index -v`
Expected: FAIL on missing `ndf.index`.

- [ ] **Step 4: Implement schema v1 and builder**

Create `projects`, `sources`, `nodes`, `node_attrs`, `aliases`, `edges`,
`diagnostics`, `baselines`, and `builds`. Insert sorted records in one
transaction, run `PRAGMA foreign_key_check`, and atomically replace output.

- [ ] **Step 5: Add a provisional scale test**

Generate 100,000 nodes and 1,000,000 edges. Run only when
`NDF_RUN_SCALE_TESTS=1`; record build time and size without setting a time gate
until real measurements establish one.

- [ ] **Step 6: Verify and commit**

Run: `python3 -m unittest tests.test_index -v`
Expected: PASS.

```bash
git add src/ndf/index.py src/ndf/sql tests
git commit -m "feat: materialize deterministic NDF indexes"
```

---

### Task 6: Add query, trace, coverage, diff, ID, and safe-fix capabilities

**Repository:** `PTO-ISA/normative_language`

**Files:**
- Create: `src/ndf/query.py`
- Create: `src/ndf/report.py`
- Create: `src/ndf/diff.py`
- Create: `src/ndf/export.py`
- Create: `src/ndf/id_allocator.py`
- Create: `src/ndf/fix.py`
- Modify: `src/ndf/cli.py`
- Create: `tests/test_query.py`
- Create: `tests/test_report.py`
- Create: `tests/test_diff.py`
- Create: `tests/test_export.py`
- Create: `tests/test_id_allocator.py`
- Create: `tests/test_fix.py`

**Interfaces:**
- Produces: `query_nodes(index, expression: str) -> tuple[NodeRecord, ...]`
- Produces: `trace(index, uri: str, edge_types: frozenset[str]) -> TraceTree`
- Produces: `coverage(index, policy: CoveragePolicy) -> CoverageReport`
- Produces: `semantic_diff(before: NdfIndex, after: NdfIndex) -> SemanticDiff`
- Produces: `export_graph(index: NdfIndex, output: Path, format: ExportFormat) -> ExportResult`
- Produces: `allocate_id(prefix: str, occupied: Iterable[str]) -> str`
- Produces: `safe_fix(paths: Sequence[Path], mode: FixMode) -> FixResult`
- `NodeRecord` is the indexed node projection; `TraceTree`, `CoverageReport`,
  `SemanticDiff`, `ExportResult`, and `FixResult` are immutable
  JSON-serializable dataclasses. `CoveragePolicy`, `ExportFormat`, and
  `FixMode` are explicit immutable configurations.

- [ ] **Step 1: Write machine-output CLI tests**

Test these experimental spellings:

```bash
ndf build --root FIXTURE --output graph.sqlite --format json
ndf check --root FIXTURE --format json
ndf show ndf://fixture/PIPE-ISSUE-001 --index graph.sqlite --format json
ndf trace PIPE-ISSUE-001 --edge refines --format json
ndf report coverage --format json
ndf diff before.sqlite after.sqlite --format json
ndf export --index graph.sqlite --output graph.json --format ndf-json
ndf id allocate PIPE-ISSUE --format json
ndf fix --generated-only --check
```

Each JSON object contains `schema_version`, `command`, `ok`, `data`, and
`diagnostics`.

- [ ] **Step 2: Write query, coverage, and trace tests**

Coverage reports active `must` L1 requirements without incoming `verifies`,
open questions/options, and `tbd` clauses. Trace terminates on cycles and
reports the cycle edge.

- [ ] **Step 3: Write semantic-diff tests**

Assert added, removed, modified, moved, superseded, and edge-only changes. A
move with the same explicit URI and body hash is `moved`, not remove/add.

- [ ] **Step 4: Implement capabilities and CLI routing**

Keep human output concise. `safe_fix` may reorder metadata and regenerate
marked navigation; return `NDF-FIX-001` if asked to alter prose, authority,
status, or semantic edges.

The `ndf-json` export is deterministic, validates against
`schemas/ndf-ir.schema.json`, includes build provenance, and omits absolute
machine-local paths. This export is the semantic before/after artifact used by
downstream move PRs.

- [ ] **Step 5: Verify and commit**

Run: `python3 -m unittest discover -s tests -p 'test_*.py'`
Expected: PASS.

```bash
git add src/ndf tests
git commit -m "feat: expose NDF graph workflows"
```

---

### Task 7: Implement the PTO ASL adapter and freeze parity fixtures

**Repository:** `PTO-ISA/normative_language`

**Files:**
- Create: `src/ndf/adapters/__init__.py`
- Create: `src/ndf/adapters/pto_asl.py`
- Create: `tests/fixtures/pto-asl/accept.asl`
- Create: `tests/fixtures/pto-asl/reject-duplicate.asl`
- Create: `tests/fixtures/pto-asl/reject-metadata.asl`
- Create: `tests/test_pto_asl_adapter.py`
- Create: `docs/adapters/pto-asl.md`

**Interfaces:**
- Produces: `parse_pto_asl(path: Path, project: ProjectManifest) -> ParseResult`
- Maps PTO `kind=intent|contract|mechanism|executable` to refinement
  `L0|L1|L2|L3`, PTO `layer` to `domain`, and PTO status to canonical lifecycle.

- [ ] **Step 1: Create redistributable parity fixtures**

Use the behavior in PTO-SPEC `tests/scripts/test_ndf.py`, preserving diagnostic
meaning without copying unrelated PTO content.

- [ ] **Step 2: Write adapter tests**

Assert region matching, comment-only bodies, metadata validation, instruction
identity generation, `[[PTO-*]]` references, and source lines.

- [ ] **Step 3: Run tests and confirm failure**

Run: `python3 -m unittest tests.test_pto_asl_adapter -v`
Expected: FAIL on missing adapter.

- [ ] **Step 4: Implement without importing PTO-SPEC code**

Keep PTO syntax policy in this adapter. Return canonical nodes and diagnostics;
do not expose PTO-specific dataclasses through the generic API.

- [ ] **Step 5: Verify and commit**

Run: `python3 -m unittest discover -s tests -p 'test_*.py'`
Expected: PASS.

```bash
git add src/ndf/adapters tests/fixtures/pto-asl tests/test_pto_asl_adapter.py docs/adapters
git commit -m "feat: add the PTO ASL NDF adapter"
```

---

### Task 7A: Index repository artifacts, evidence, and pinned dependencies

**Repository:** `PTO-ISA/normative_language`

**Files:**
- Create: `src/ndf/adapters/repository.py`
- Create: `src/ndf/adapters/evidence.py`
- Create: `src/ndf/dependencies.py`
- Create: `schemas/ndf-evidence.schema.json`
- Create: `tests/fixtures/repository/src/core.py`
- Create: `tests/fixtures/repository/tests/test_core.py`
- Create: `tests/fixtures/repository/evidence/core.json`
- Create: `tests/test_repository_adapter.py`
- Create: `tests/test_evidence_adapter.py`
- Create: `tests/test_dependencies.py`

**Interfaces:**
- Produces: `index_repository_paths(root: Path, globs: Sequence[str], project: ProjectManifest) -> ParseResult`
- Produces: `parse_evidence_manifest(path: Path, project: ProjectManifest) -> ParseResult`
- Produces: `resolve_dependencies(manifest: ProjectManifest, lock: ProjectLock, workspace: Path) -> tuple[ResolvedProject, ...]`
- `ResolvedProject` records canonical project ID, local path, exact Git revision,
  dirty-state policy, and dependency provenance.

- [ ] **Step 1: Write source, test, and evidence fixture tests**

Assert deterministic derived IDs for source and test artifacts, explicit IDs
for evidence records, and `implements`, `verifies`, and `evidenced-by` edges to
normative clauses. Reordering the input globs must not change identities or
serialized output.

- [ ] **Step 2: Write dependency resolution tests**

Cover an exact detached commit, a submodule at the locked revision, a missing
dependency, a symbolic branch name, a revision mismatch, and a dirty dependency
when `allow_dirty` is false. Assert stable diagnostics:

```text
NDF-DEP-001 NDF-DEP-002 NDF-DEP-003 NDF-DEP-004
```

- [ ] **Step 3: Run tests and confirm failure**

Run: `python3 -m unittest tests.test_repository_adapter tests.test_evidence_adapter tests.test_dependencies -v`
Expected: FAIL on missing adapter and dependency modules.

- [ ] **Step 4: Implement the bounded repository adapters**

Repository paths are descriptive nodes, not normative clauses. Derive their
IDs from `(project ID, normalized repository-relative path, artifact kind)`;
store Git revision and content hash as attributes. Evidence manifests validate
against `schemas/ndf-evidence.schema.json` and must name the command, result,
tool version, source revision, timestamp, and linked clause IDs.

Dependency resolution reads only manifest paths and exact revisions from
`ndf.lock`. It may inspect a checked-out Git worktree or submodule, but it must
not clone, fetch, or mutate repositories. Add `ndf report dependencies
--format json` so CI can record the resolved compatibility tuple.

- [ ] **Step 5: Verify and commit**

Run: `python3 -m unittest discover -s tests -p 'test_*.py'`
Expected: PASS.

```bash
git add src/ndf/adapters src/ndf/dependencies.py schemas tests
git commit -m "feat: index repository evidence and pinned dependencies"
```

---

### Task 8: Revise the NDF specification and publish experimental release 0.1.0

**Repository:** `PTO-ISA/normative_language`

**Files:**
- Modify: `normative_language.md`
- Modify: `normative_language_cn.md`
- Create: `docs/format/migration-v0.1-to-v0.2.md`
- Create: `CHANGELOG.md`
- Create: `tests/test_spec_sync.py`
- Modify: `README.md`

**Interfaces:**
- Produces format `0.2`, IR `0.1`, and machine-output schema `0.1`.
- Produces Git tag `v0.1.0` after merge and CI success.

- [ ] **Step 1: Write specification contract tests**

Require both language files to carry matching versions and sections for derived
indexes, canonical dimensions, namespaced identity, front matter, lock files,
agent enforcement, and maturity levels.

- [ ] **Step 2: Run the sync test and confirm failure**

Run: `python3 -m unittest tests.test_spec_sync -v`
Expected: FAIL because the new sections are absent.

- [ ] **Step 3: Update English and Chinese specifications together**

Replace “no database” with derived-index rules. Replace overloaded fields with
`kind/modality/refinement/domain/status`. Document legacy adapters, `ndf://`
URIs, file front matter, `ndf.lock`, maturity, JSON output, and agent gates.

- [ ] **Step 4: Write migration guide and changelog**

Include before/after DavinciOO Markdown and PTO ASL mappings. State that CLI
spelling and plugin ABI remain experimental.

- [ ] **Step 5: Run the release-candidate gate**

```bash
python3 -m unittest discover -s tests -p 'test_*.py'
ndf check --root tests/fixtures/markdown/project --format json
git diff --check
```

Expected: PASS and no untracked SQLite database.

- [ ] **Step 6: Commit, merge through PR, then tag**

```bash
git add normative_language*.md README.md CHANGELOG.md docs/format tests/test_spec_sync.py
git commit -m "docs: define NDF knowledge compiler format 0.2"
git tag -a v0.1.0 -m "NDF knowledge compiler experimental 0.1.0"
```

Push the tag only after it points to the merged release commit.

---

### Task 9: Pin NDF in PTO-SPEC, prove parity, and publish compatibility patch 0.1.1

**Repository:** `PTO-ISA/pto-spec`

**Files:**
- Modify: `.gitmodules`
- Create gitlink: `tools/ndf`
- Create: `ndf.yaml`
- Create: `ndf.lock`
- Create: `tests/scripts/test_ndf_adapter_parity.py`
- Create: `tests/fixtures/ndf-parity/expected.json`
- Modify: `README.md`

**Interfaces:**
- Consumes: `PTO-ISA/normative_language` tag `v0.1.0` at an exact commit.
- Produces parity evidence between `scripts.ndf.check_repository()` and the new
  ASL adapter over the complete repository.
- Produces NDF tag `v0.1.1` containing all parity fixes and evidence; the PTO PR
  finishes pinned to this tag.

- [ ] **Step 1: Add the exact NDF submodule pin**

```bash
git submodule add https://github.com/PTO-ISA/normative_language.git tools/ndf
git -C tools/ndf checkout v0.1.0
```

Record the resolved 40-hex commit in `ndf.lock`; configure project `pto-spec`,
ASL roots, `PTO-*` prefixes, and the PTO ASL adapter.

- [ ] **Step 2: Write the failing full-tree parity test**

Load `scripts/ndf.py` as old oracle and `tools/ndf/src/ndf` as candidate.
Compare canonical clause IDs, bodies, paths, lines, references, and normalized
validation errors. Keep field translation explicit in the test.

- [ ] **Step 3: Run the parity test**

Run: `python3 -m unittest tests.scripts.test_ndf_adapter_parity -v`
Expected initially: FAIL with a structured parity-gap list, not import or
network failures.

- [ ] **Step 4: Fix generic gaps upstream and publish NDF 0.1.1**

For each generic gap, add a minimal upstream fixture. In the NDF repository,
record the PTO parity tuple in `compatibility/pto-spec.yaml`, merge the patch,
and tag the merge commit `v0.1.1` even if parity required documentation/evidence
only. Return to PTO-SPEC, update `tools/ndf` and `ndf.lock` to `v0.1.1`, and
rerun. Keep PTO-only repository hygiene in PTO.

- [ ] **Step 5: Verify and commit**

Run:

```bash
python3 -m unittest tests.scripts.test_ndf_adapter_parity -v
make pr-check
```

Expected: PASS.

```bash
git add .gitmodules tools/ndf ndf.yaml ndf.lock tests README.md
git commit -m "test: prove generic NDF parity for PTO ASL"
```

---

### Task 10: Switch PTO-SPEC `check-ndf` to the generic engine

**Repository:** `PTO-ISA/pto-spec`

**Files:**
- Modify: `scripts/check-ndf`
- Rename: `scripts/ndf.py` to `scripts/ndf_legacy.py`
- Modify: `tests/scripts/test_ndf.py`
- Modify: `tests/scripts/test_ndf_adapter_parity.py`
- Modify: `Makefile`
- Modify: `.github/workflows/asl.yml`
- Create: `docs/development/ndf.md`

**Interfaces:**
- `./scripts/check-ndf` remains the stable PTO repository command.
- The wrapper invokes the pinned engine with `ndf.yaml` and `ndf.lock`.
- PTO-specific publication/legacy rules remain namespaced PTO policy.

- [ ] **Step 1: Make the generic adapter primary in tests**

Keep all current accepted/rejected cases. Add a subprocess test for
`./scripts/check-ndf` that verifies exit 0/1 and the expected diagnostic code.

- [ ] **Step 2: Run and confirm the wrapper contract fails**

Run: `python3 -m unittest tests.scripts.test_ndf -v`
Expected: FAIL only on the new wrapper contract.

- [ ] **Step 3: Replace the wrapper**

Import the pinned source locally:

```python
NDF_SRC = ROOT / "tools" / "ndf" / "src"
sys.path.insert(0, str(NDF_SRC))
from ndf.cli import main as ndf_main
raise SystemExit(ndf_main(["check", "--root", str(ROOT), "--format", "human"]))
```

Keep `ndf_legacy.py` for one experimental compatibility window. Parity tests
run it; normal CI does not.

- [ ] **Step 4: Update documentation and checkout gates**

Document pin updates. Ensure CI initializes `tools/ndf` recursively and
installs the pinned runtime before `make pr-check`:

```bash
python3 -m pip install -r tools/ndf/requirements-lock.txt
python3 -m pip install -e tools/ndf --no-deps
```

`make pr-check` must still run `./scripts/check-ndf`.

- [ ] **Step 5: Verify and commit**

Run: `make pr-check`
Expected: PASS with no ASL, catalog, architecture, or generated-doc change.

```bash
git add scripts tests Makefile .github docs/development/ndf.md
git commit -m "refactor: use the generic NDF validator"
```

---

### Task 11: Add DavinciOO in-place indexing and a diagnostic baseline

**Repository:** `hengliao1972/DavinciOO`

**Prerequisite:** PTO-SPEC migration is merged and `isa/` points to a commit
using the generic adapter.

**Files:**
- Modify: `.gitmodules`
- Create gitlink: `toolchains/ndf`
- Create: `ndf.yaml`
- Create: `ndf.lock`
- Create: `ndf-baseline.json`
- Create: `toolchains/scripts/build_ndf_index.py`
- Create: `srcs/tests/unit/test_ndf_project_contract.py`
- Modify: `.gitignore`
- Modify: `toolchains/README.md`

**Interfaces:**
- Consumes exact `normative_language` and `isa/` commits.
- Produces `.ndf-cache/davincioo.sqlite` locally, never tracked.
- Produces hard no-regression comparison with `ndf-baseline.json`.

- [ ] **Step 1: Write the failing project-contract test**

Assert submodule commits equal lock entries, project is `davincioo`, PTO
resolves through `isa/`, baseline entries have owner/milestone, and no tracked
path ends in `.sqlite`.

- [ ] **Step 2: Run and confirm missing configuration**

Run: `pytest srcs/tests/unit/test_ndf_project_contract.py -q`
Expected: FAIL because tool pin, manifest, lock, and baseline are absent.

- [ ] **Step 3: Add the NDF pin and manifest**

```bash
git submodule add https://github.com/PTO-ISA/normative_language.git toolchains/ndf
git -C toolchains/ndf checkout v0.1.1
```

Configure roots for `designs/DavinciOO/ndf`, `docs`, `srcs/core`, `srcs/tests`,
and curated evidence. Configure `isa/` as project `pto-spec`.

- [ ] **Step 4: Generate and review the first baseline**

Classify accepted pre-existing diagnostics with owner and one milestone:

```text
M1-index-adoption M2-authority-migration M3-hierarchy-move M4-debt-zero
```

Never baseline duplicate explicit IDs, invalid locks, or non-determinism.

- [ ] **Step 5: Implement the build wrapper**

Import only `toolchains/ndf/src`, support `--check`, `--changed`, and `--output`,
write under `.ndf-cache/` by default, and fail on new/worsened diagnostics.

- [ ] **Step 6: Verify and commit**

```bash
python3 -m pip install -r toolchains/ndf/requirements-lock.txt
python3 -m pip install -e toolchains/ndf --no-deps
python3 toolchains/scripts/build_ndf_index.py --check
pytest srcs/tests/unit/test_ndf_project_contract.py -q
git diff --check
```

Expected: PASS.

```bash
git add .gitmodules toolchains/ndf ndf.yaml ndf.lock ndf-baseline.json toolchains/scripts/build_ndf_index.py srcs/tests/unit/test_ndf_project_contract.py .gitignore toolchains/README.md
git commit -m "feat: index DavinciOO design knowledge"
```

---

### Task 12: Enforce the index contract for agents and CI

**Repository:** `hengliao1972/DavinciOO`

**Files:**
- Modify: `AGENTS.md`
- Modify: `.claude/CLAUDE.md`
- Modify: `.claude/rules/repository.md`
- Modify: `.github/workflows/ci.yml`
- Modify: `.github/pull_request_template.md`
- Modify: `.pre-commit-config.yaml`
- Create: `srcs/tests/unit/test_ndf_agent_contract.py`

**Interfaces:**
- Changed scope: `python3 toolchains/scripts/build_ndf_index.py --changed --check`
- Full gate: `python3 toolchains/scripts/build_ndf_index.py --check`

- [ ] **Step 1: Write the failing agent-contract test**

Require all guidance files to contain both commands, prohibit SQLite edits,
require stable IDs for durable nodes, and prohibit semantic safe-fix. Require
the full gate in the PR template.

- [ ] **Step 2: Run and confirm guidance is incomplete**

Run: `pytest srcs/tests/unit/test_ndf_agent_contract.py -q`
Expected: FAIL with missing contract phrases.

- [ ] **Step 3: Add guidance and the local hook**

Add a local pre-commit hook for changed docs/NDF/config. It runs changed-scope
validation and never writes files. Configure the hook as an isolated Python
hook with `PyYAML==6.0.2` as an additional dependency. Keep full validation in
CI.

- [ ] **Step 4: Add the CI job**

Checkout recursive submodules, use Python 3.12, install
`toolchains/ndf/requirements-lock.txt` plus the local NDF package with
`--no-deps`, then run project-contract tests and full index check. Upload JSON
diagnostics on failure, not SQLite.

- [ ] **Step 5: Verify and commit**

```bash
pytest srcs/tests/unit/test_ndf_agent_contract.py srcs/tests/unit/test_ndf_project_contract.py -q
pre-commit run --all-files
python3 toolchains/scripts/build_ndf_index.py --check
```

Expected: PASS.

```bash
git add AGENTS.md .claude .github .pre-commit-config.yaml srcs/tests/unit/test_ndf_agent_contract.py
git commit -m "ci: enforce the DavinciOO NDF index contract"
```

---

### Task 13: Move DavinciOO NDF into the canonical documentation tree

**Repository:** `hengliao1972/DavinciOO`

**Files:**
- Move: `designs/DavinciOO/ndf/` to `docs/specification/davincioo/ndf/`
- Move: `designs/DavinciOO/superscalar_v5.md` to `docs/architecture/core/superscalar.md`
- Move: `designs/DavinciOO/superscalar_v5_warp.md` to `docs/architecture/core/multi-warp.md`
- Move: `designs/DavinciOO/superscalar_v5_warp_versioned_arch_id.md` to `docs/architecture/core/versioned-architecture-id.md`
- Move: `designs/DavinciOO/diff_v5_over_v4.md` to `docs/archive/architecture/core/diff-v5-over-v4.md`
- Move: `designs/DavinciOO/old/` to `docs/archive/architecture/core/`
- Move: `designs/DavinciOO/pic/` to `docs/architecture/assets/core/`
- Modify: `ndf.yaml`
- Modify: moved links, NDF source declarations, and NDF skill paths
- Create: `srcs/tests/unit/test_ndf_identity_preservation.py`

**Interfaces:**
- Explicit document and clause URIs remain unchanged.
- Semantic diff reports moves/reference rewrites, not removals, additions, or
  clause-body changes.

- [ ] **Step 1: Capture the pre-move index**

```bash
python3 toolchains/scripts/build_ndf_index.py --output /tmp/davincioo-before.sqlite
```

Record semantic hash and explicit-node census in the test fixture.

- [ ] **Step 2: Write the failing identity-preservation test**

Compare the current graph with the saved pre-move export. Allow source-path and
relative-link changes only; reject URI, body hash, authority, status, edge, or
owner changes.

- [ ] **Step 3: Move with `git mv` and rewrite links**

Perform only the mappings above. Update nearby READMEs,
`.claude/skills/ndf-spec/SKILL.md`, and all matches from:

```bash
rg -n 'designs/DavinciOO/(ndf|superscalar_v5|old|pic)' . --glob '!isa/**'
```

- [ ] **Step 4: Run semantic diff before prose edits**

```bash
python3 toolchains/scripts/build_ndf_index.py --output /tmp/davincioo-after.sqlite
PYTHONPATH=toolchains/ndf/src python3 -m ndf diff /tmp/davincioo-before.sqlite /tmp/davincioo-after.sqlite --format json
```

Expected: move and reference-location changes only. Fix semantic changes before
continuing.

- [ ] **Step 5: Verify and commit the move separately**

```bash
pytest srcs/tests/unit/test_ndf_identity_preservation.py -q
python3 toolchains/scripts/build_ndf_index.py --check
pre-commit run --all-files
```

Expected: PASS.

```bash
git add -A docs designs ndf.yaml .claude srcs/tests/unit/test_ndf_identity_preservation.py
git commit -m "docs: move DavinciOO NDF into the canonical tree"
```

---

### Task 14: Reorganize explanatory docs and generate one navigation view

**Repository:** `hengliao1972/DavinciOO`

**Files:**
- Read input: `/Users/zhoubot/Downloads/agentic_circuit_optimizer.md`
- Read input: `/Users/zhoubot/Downloads/agentic_tao_physical_design_flow.md`
- Read duplicate check only: `/Users/zhoubot/Downloads/agentic_circuit_optimizer(1).md`
- Move: `docs/superscalar/` to `docs/architecture/core/history/`
- Move: `docs/vector/` to `docs/architecture/execution/vector/`
- Move: `docs/cube/` to `docs/architecture/execution/cube/`
- Move: `docs/interleave/` to `docs/architecture/execution/interleave/`
- Move: `docs/tregfile/` to `docs/architecture/memory/tregfile/`
- Move: `docs/tlsu/` to `docs/architecture/memory/tlsu/`
- Move: `docs/superpowers/specs/` to `docs/development/change-history/specs/`
- Move: `docs/superpowers/plans/` to `docs/development/change-history/plans/`
- Create: `docs/verification/README.md`
- Create: `docs/verification/pto-conformance.md`
- Create: `docs/verification/commit-trace-contract.md`
- Create: `docs/optimization/README.md`
- Create: `docs/optimization/core-loop/method.md`
- Create: `docs/physical-design/README.md`
- Create: `docs/physical-design/tao/README.md`
- Create: `docs/physical-design/tao/flow.md`
- Move: `docs/top/README.md` to `docs/archive/navigation/top-readme.md`
- Modify: `docs/README.md`
- Modify: `docs/governance/design-rules.md`
- Create: `toolchains/scripts/generate_docs_navigation.py`
- Create: `srcs/tests/unit/test_docs_navigation.py`

**Interfaces:**
- `docs/README.md` is the only canonical human navigation root.
- Generated blocks use `<!-- ndf:index:start -->` and
  `<!-- ndf:index:end -->` and deterministic indexed ordering.

- [ ] **Step 1: Add document metadata before moving domains**

Give each current entrypoint an explicit `doc_id`, owner, lifecycle, and
authority. Mark old revisions and PDFs historical/informative. Do not assign
explicit IDs to every image or generated PDF.

- [ ] **Step 2: Write navigation tests before moves**

Assert one canonical root, one current document per topic, no current link to a
historical node, no missing local target, and stable generated output.

- [ ] **Step 3: Run tests and record current failures**

Run: `pytest srcs/tests/unit/test_docs_navigation.py -q`
Expected: FAIL for duplicate `docs/top` authority, missing `docs/tma`, and
missing TRegFile targets observed in the current tree.

- [ ] **Step 4: Move one domain at a time**

After each mapping, run navigation tests, index check, and `git diff --check`.
Do not proceed to the next mapping with broken links.

- [ ] **Step 5: Add lifecycle areas without copying authority**

`verification` explains conformance/equivalence/coverage; `optimization`
explains front-end and end-to-end loops; `physical-design/tao` explains
RTL-to-layout. Normative constraints and decisions stay in NDF and are linked.

Adapt the reviewed Agentic Core Optimizer article into
`optimization/core-loop/method.md` with document ID
`DOC-DAVINCIOO-OPT-CORE-LOOP`. Replace ARM-specific authority wording with the
pinned PTO-SPEC ASL and link the DavinciOO NDF constraints, decisions, and
verification clauses. Adapt the reviewed TAO article into
`physical-design/tao/flow.md` with ID `DOC-DAVINCIOO-PD-TAO-FLOW`. Do not import
`agentic_circuit_optimizer(1).md`; it duplicates the core article except for
one related-document link.

Create `pto-conformance.md` as the readable authority/mapping guide and
`commit-trace-contract.md` as the readable refinement-observation contract.
Both are informative views that link PTO ASL and NDF verification clauses.
Move agent workflow plans/specs under development change history so they cannot
appear as product architecture entrypoints.

- [ ] **Step 6: Generate root navigation and archive `docs/top`**

The generator reads SQLite, updates delimited blocks only, supports `--check`,
and never rewrites surrounding prose.

- [ ] **Step 7: Verify and commit**

```bash
pytest srcs/tests/unit/test_docs_navigation.py srcs/tests/unit/test_ndf_identity_preservation.py -q
python3 toolchains/scripts/generate_docs_navigation.py --check
python3 toolchains/scripts/build_ndf_index.py --check
pre-commit run --all-files
```

Expected: PASS and no dangling local link.

```bash
git add docs toolchains/scripts/generate_docs_navigation.py srcs/tests/unit/test_docs_navigation.py ndf-baseline.json
git commit -m "docs: organize DavinciOO by authority and lifecycle"
```

---

### Task 15: Close migration debt and publish compatibility evidence

**Repositories:** all three, in upstream-to-downstream order

**Files:**
- Create in NDF: `compatibility/pto-spec.yaml`
- Create in NDF: `compatibility/davincioo.yaml`
- Create in NDF: `docs/releases/compatibility-v0.2.md`
- Modify in NDF: `CHANGELOG.md`
- Modify in PTO-SPEC: `ndf.lock`
- Modify in DavinciOO: `ndf.lock`
- Modify in DavinciOO: `ndf-baseline.json`

**Interfaces:**
- Produces exact NDF, PTO-SPEC, and DavinciOO compatibility commits.
- Promotes only measured format/IR/JSON surfaces to provisional. CLI spelling
  and plugin ABI stay experimental without separate evidence.

- [ ] **Step 1: Run the three-repository matrix**

For the candidate NDF commit, run all NDF tests, PTO `make pr-check`, and
DavinciOO NDF/navigation/unit gates in clean recursive-submodule worktrees.

- [ ] **Step 2: Record exact evidence**

Each YAML records repository URL, 40-hex commit, commands, status, semantic
index hash, node/edge/diagnostic census, and UTC run date. Exclude local paths.

- [ ] **Step 3: Remove expired baseline entries**

Fix source or extend an entry only with owner, milestone, rationale, and a
decision record. Duplicate IDs and non-determinism remain non-waivable.

- [ ] **Step 4: Advance pins in dependency order**

Release NDF, update/merge PTO-SPEC, then update DavinciOO NDF and `isa/`
gitlinks. Rerun downstream gates after every pin change.

- [ ] **Step 5: Commit evidence and release notes**

```bash
git add compatibility docs/releases CHANGELOG.md
git commit -m "docs: publish NDF cross-repository compatibility"
```

Commit downstream lock/gitlink updates separately with the consumed NDF version
in each commit message.

---

## Final Verification Gate

Run from clean worktrees after upstream pins merge:

```bash
# PTO-ISA/normative_language
python3 -m pip install -r requirements-lock.txt
python3 -m pip install -e . --no-deps
python3 -m unittest discover -s tests -p 'test_*.py'
ndf check --root tests/fixtures/markdown/project --format json

# PTO-ISA/pto-spec
git submodule update --init --recursive
python3 -m pip install -r tools/ndf/requirements-lock.txt
python3 -m pip install -e tools/ndf --no-deps
make pr-check

# hengliao1972/DavinciOO
git submodule update --init --recursive
python3 -m pip install -e ".[dev]"
python3 -m pip install -r toolchains/ndf/requirements-lock.txt
python3 -m pip install -e toolchains/ndf --no-deps
python3 toolchains/scripts/build_ndf_index.py --check
python3 toolchains/scripts/generate_docs_navigation.py --check
pytest srcs/tests/unit -q
pre-commit run --all-files
```

Completion requires:

- all commands pass from clean checkouts;
- SQLite semantic hashes are stable;
- PTO old/new NDF parity exists for the compatibility window;
- DavinciOO has no new or worsened diagnostic;
- moved explicit identities are preserved;
- `docs/README.md` is the only canonical current-design navigation root;
- compatibility files record exact commits for all repositories;
- no database, local override, cache, or symbolic commit placeholder is tracked.
