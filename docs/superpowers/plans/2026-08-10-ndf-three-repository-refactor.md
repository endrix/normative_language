# NDF Three-Repository Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the NDF knowledge compiler in `PTO-ISA/normative_language`, migrate `PTO-ISA/pto-spec` to its ASL adapter without changing architecture authority, then use the indexed graph to reorganize and hard-gate the DavinciOO documentation corpus.

**Architecture:** Markdown, ASL, source, test, and evidence files remain Git-tracked sources of truth. Parser adapters normalize them into a versioned canonical IR, generic and project rules validate that IR, and a deterministic SQLite database provides disposable query indexes. The dependency direction is strictly `DavinciOO -> normative_language`, `DavinciOO -> pto-spec`, and `pto-spec -> normative_language`; no upstream repository imports DavinciOO.

**Tech Stack:** Rust 1.94.0, Cargo edition 2024, Clap, Serde, Rusqlite with bundled SQLite, Markdown, Git, Git submodules, GitHub Actions; repository-local Python tests may invoke the NDF binary but NDF itself has no Python runtime.

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

## Rust Compiler Foundation

Before Task 9, execute
[`2026-08-10-ndf-rust-compiler-migration.md`](2026-08-10-ndf-rust-compiler-migration.md)
in full. The first `v0.1.0` tag points to its merged Rust release commit.

Downstream repositories build NDF with `cargo build --locked --release` and call
its versioned JSON CLI. NDF has no Python runtime or import surface.

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

### Completed foundation: Rust NDF v0.1.0

The standalone Rust compiler, frozen golden contracts, deterministic SQLite index, complete JSON CLI, and release-scale evidence are implemented in `PTO-ISA/normative_language`. The release PR must merge before the exact merged commit is tagged `v0.1.0`; downstream work begins only after that tag exists.

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

Load `scripts/ndf.py` as the old oracle. Build the pinned Rust candidate with
`cargo build --manifest-path tools/ndf/Cargo.toml --locked --release` and invoke
`tools/ndf/target/release/ndf` through `subprocess`. Compare canonical clause
IDs, bodies, paths, lines, references, and normalized validation errors. Keep
field translation explicit in the test; do not import candidate internals.

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

Make `scripts/check-ndf` execute the pinned binary:

```bash
cargo build --manifest-path "$ROOT/tools/ndf/Cargo.toml" --locked --release -p ndf-cli
exec "$ROOT/tools/ndf/target/release/ndf" check --root "$ROOT" --format json
```

Keep `ndf_legacy.py` for one experimental compatibility window. Parity tests
run it; normal CI does not.

- [ ] **Step 4: Update documentation and checkout gates**

Document pin updates. Ensure CI initializes `tools/ndf` recursively and builds
the pinned runtime before `make pr-check`:

```bash
cargo build --manifest-path tools/ndf/Cargo.toml --locked --release -p ndf-cli
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

Invoke `toolchains/ndf/target/release/ndf` as a subprocess, parse only its
versioned JSON envelope, support `--check`, `--changed`, and `--output`, write
under `.ndf-cache/` by default, and fail on new/worsened diagnostics. Do not
import or duplicate NDF compiler logic.

- [ ] **Step 6: Verify and commit**

```bash
cargo build --manifest-path toolchains/ndf/Cargo.toml --locked --release -p ndf-cli
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
validation and never writes files. Configure it as a repository-local system
hook that calls the project wrapper; the wrapper invokes the pinned Rust binary.
Keep full validation in CI.

- [ ] **Step 4: Add the CI job**

Checkout recursive submodules, install Rust 1.94.0 and Python 3.12, build NDF
with `cargo build --manifest-path toolchains/ndf/Cargo.toml --locked --release`,
then run project-contract tests and the full index check. Upload JSON diagnostics
on failure, not SQLite.

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
toolchains/ndf/target/release/ndf diff /tmp/davincioo-before.sqlite /tmp/davincioo-after.sqlite --format json
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
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --locked
cargo build --workspace --release --locked
scripts/ndf check --root tests/fixtures/markdown/project --format json

# PTO-ISA/pto-spec
git submodule update --init --recursive
cargo build --manifest-path tools/ndf/Cargo.toml --locked --release -p ndf-cli
make pr-check

# hengliao1972/DavinciOO
git submodule update --init --recursive
python3 -m pip install -e ".[dev]"
cargo build --manifest-path toolchains/ndf/Cargo.toml --locked --release -p ndf-cli
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
