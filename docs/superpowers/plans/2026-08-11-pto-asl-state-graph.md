# PTO ASL State Graph Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish a typed, queryable PTO ASL unit and architecture-state graph in NDF, rewrite PTO-SPEC PR 66 to provide that graph, and update DavinciOO to consume the resulting state identities.

**Architecture:** The Rust PTO adapter emits source-unit, synthetic-unit, architecture-state, instruction, and explicit clause nodes into the existing canonical IR. PTO-SPEC authors `PTO-STATE` records beside the owning ASL declarations and pins the merged compiler; DavinciOO consumes only merged PTO state, instruction, and clause identities through exact `conforms-to` edges. ASL remains the only source of PTO semantics, while SQLite remains a disposable index.

**Tech Stack:** Rust 1.94.0, Cargo edition 2024, Serde/Serde JSON, Rusqlite, Clap, Python 3.12 standard library and unittest/pytest, ASL1, YAML manifests, Git submodules, GitHub Actions.

## Global Constraints

- Execute inline in the existing isolated worktrees; do not use subagents.
- Follow red-green-refactor for every production behavior change.
- Preserve canonical IR schema `0.1` and SQLite schema `1`; add no `NodeKind` variant.
- Use `NodeKind::Source` plus `pto_entity=unit|synthetic-unit` for ASL ownership and `NodeKind::Architecture` plus `pto_entity=state` for state families.
- Keep PTO executable ASL behavior, instruction encodings, legality, faults, completion, ordering, and profiles unchanged.
- Treat `PTO-UNIT` as source provenance, never as a conformance requirement.
- Keep `PTO-INSTRUCTION`-derived `PTO-INST-*` nodes as executable ISA identities.
- Keep state requirements and `PTO-STATE-*` identities separate and linked.
- Do not commit SQLite databases, `build/`, `.cache/`, `target/`, or machine-local state.
- Advance downstream pins only to merged 40-character commits.
- Preserve PTO-SPEC PR 66's release-event schema as a separate commit and describe it as a delivery contract, not PTO state semantics.
- Run repository-specific full gates before pushing each branch.

---

## Repository and Merge Sequence

| Order | Repository | Branch | Required checkpoint |
| --- | --- | --- | --- |
| 1 | `PTO-ISA/normative_language` | `codex/pto-asl-state-graph` | Merge the compiler PR and record its merge commit |
| 2 | `PTO-ISA/pto-spec` | `codex/pto-release-dispatch-a` / PR 66 | Pin the merged compiler commit, rewrite state graph, merge PR 66 |
| 3 | `hengliao1972/DavinciOO` | `codex/ndf-docs-refactor` | Pin both merged commits and update conformance mappings |

Do not commit a downstream gitlink or `ndf.lock` revision that names an
unmerged upstream commit. Local uncommitted worktree paths may be used only for
pre-merge integration tests.

---

### Task 1: Emit ASL source-unit and instruction provenance nodes

**Repository:** `PTO-ISA/normative_language`

**Files:**

- Modify: `crates/ndf-compiler/src/adapters/pto_asl.rs`
- Modify: `crates/ndf-compiler/src/adapters/mod.rs`
- Modify: `crates/ndf-compiler/tests/pto_asl.rs`
- Modify: `tests/fixtures/pto-asl/accept.asl`
- Add: `tests/fixtures/pto-asl/reject-unit.asl`
- Modify: `tests/golden/pto-asl.json`

**Interfaces:**

- Consumes a complete `PTO-UNIT` JSON object or the unit fields inside one `PTO-INSTRUCTION` object.
- Produces `NodeKind::Source` unit nodes with `pto_entity=unit`, `surface`, `classification`, and optional `mnemonic` attributes.
- Produces `defined-in` from every `PTO-INST-*` instruction node to its unit node.
- Maps exact dependency `generated:decoders` to `PTO-SYNTHETIC-GENERATED-DECODERS`.

- [ ] **Step 1: Expand the accepted fixture and write failing assertions**

Replace the first fixture line with a complete instruction unit record:

```asl
// PTO-INSTRUCTION: {"id":"PTO-TILE-TLOAD","surface":"tile","classification":["memory-and-data-movement","regular"],"depends_on":["PTO-TILE-MODEL-MEMORY-LOAD-STORE","generated:decoders"],"mnemonic":"TLOAD","catalog_records":[{"semantic_handler":"TLOAD","contract_status":"reviewed-complete","state_effects":["operand:destination0:destination"]}]}
```

Add assertions in `instruction_identity_and_clause_contracts_are_preserved`:

```rust
let unit = parsed.nodes.iter().find(|node| node.id.local_id() == "PTO-TILE-TLOAD").unwrap();
assert_eq!(unit.kind, NodeKind::Source);
assert_eq!(unit.attributes["pto_entity"], "unit");
assert!(parsed.edges.iter().any(|edge| {
    edge.edge_type == "defined-in"
        && edge.source.local_id() == "PTO-INST-TILE-TLOAD"
        && edge.target.local_id() == "PTO-TILE-TLOAD"
}));
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```bash
cargo test -p ndf-compiler --test pto_asl instruction_identity_and_clause_contracts_are_preserved --locked
```

Expected: FAIL because only the instruction and clause nodes exist.

- [ ] **Step 3: Implement strict unit parsing**

Add constants and helpers with these signatures:

```rust
const UNIT_PREFIX: &str = "// PTO-UNIT: ";
const SYNTHETIC_DECODER_ALIAS: &str = "generated:decoders";
const SYNTHETIC_DECODER_ID: &str = "PTO-SYNTHETIC-GENERATED-DECODERS";

fn unit_node(
    metadata: &Map<String, Value>,
    path: &str,
    line: u32,
    project: &ProjectManifest,
) -> Result<Result<Node, Diagnostic>, CompilerError>;

fn unit_dependency_id(
    value: &str,
    project: &ProjectManifest,
) -> Result<NodeId, CompilerError>;
```

Require exact string `id`, exact string `surface`, non-empty string arrays
`classification` and `depends_on`, and `mnemonic` for instruction records.
Create unit `depends-on` edges and one synthetic source node per project only
when the alias is referenced. Reject any other non-`PTO-*` dependency with
`NDF-ASL-006`.

- [ ] **Step 4: Add negative unit metadata cases**

Put malformed JSON, missing `id`, duplicate unit metadata records, and an
unsupported `generated:other` dependency in `reject-unit.asl`. Assert the
resulting diagnostic codes and repairs in the frozen oracle test.

- [ ] **Step 5: Run focused tests and regenerate the golden file deliberately**

Run:

```bash
cargo test -p ndf-compiler --test pto_asl --locked
```

Expected before the golden update: FAIL with an actual/expected JSON diff.
Review every emitted node and edge, then update only
`tests/golden/pto-asl.json` and rerun. Expected after update: PASS.

- [ ] **Step 6: Commit the independently reviewable unit graph**

```bash
git add crates/ndf-compiler/src/adapters tests/fixtures/pto-asl tests/golden/pto-asl.json crates/ndf-compiler/tests/pto_asl.rs
git commit -m "feat(pto): index ASL unit provenance"
```

---

### Task 2: Parse and validate first-class PTO state nodes

**Repository:** `PTO-ISA/normative_language`

**Files:**

- Modify: `crates/ndf-compiler/src/adapters/pto_asl.rs`
- Modify: `crates/ndf-compiler/tests/pto_asl.rs`
- Modify: `tests/fixtures/pto-asl/accept.asl`
- Add: `tests/fixtures/pto-asl/reject-state.asl`
- Modify: `tests/golden/pto-asl.json`

**Interfaces:**

- Consumes `// PTO-STATE: <JSON>` records with exact fields `id`, `classification`, `scope`, `owner`, `members`, and `depends_on`.
- Produces `NodeKind::Architecture` with `pto_entity=state`, deterministic state attributes, `defined-in`, `owns-state`, and state `depends-on` edges.
- Emits `NDF-ASL-008` for malformed records and `NDF-ASL-009` for same-file owner/member violations.

- [ ] **Step 1: Add a valid state fixture and failing test**

Append to `accept.asl` before the NDF regions:

```asl
// PTO-STATE: {"id":"PTO-STATE-TILE-LOCAL","classification":["tile","local"],"scope":"core","owner":"PTO-TILE-TLOAD","members":["_Tiles","_TileAllocationMasks"],"depends_on":[]}
var _Tiles : array [[64]] of bits(32);
var _TileAllocationMasks : array [[64]] of bits(4);
```

Add this test:

```rust
#[test]
fn state_metadata_emits_architecture_node_and_provenance_edges() {
    let root = repository().join("tests/fixtures/pto-asl");
    let project = ProjectManifest::for_test("pto-spec", &root, ["PTO"], ["tile"]);
    let parsed = parse_pto_asl(&root.join("accept.asl"), &project).unwrap();
    let state = parsed.nodes.iter()
        .find(|node| node.id.local_id() == "PTO-STATE-TILE-LOCAL")
        .unwrap();
    assert_eq!(state.kind, NodeKind::Architecture);
    assert_eq!(state.attributes["pto_entity"], "state");
    assert_eq!(state.attributes["scope"], "core");
    assert_eq!(state.attributes["members"], "_Tiles,_TileAllocationMasks");
}
```

- [ ] **Step 2: Run and verify RED**

Run:

```bash
cargo test -p ndf-compiler --test pto_asl state_metadata_emits_architecture_node_and_provenance_edges --locked
```

Expected: FAIL because `PTO-STATE` records are ignored.

- [ ] **Step 3: Implement the state parser**

Add:

```rust
const STATE_PREFIX: &str = "// PTO-STATE: ";
const STATE_SCOPES: &[&str] = &["pe", "bundle", "core", "memory-agent", "acr", "system"];

fn state_node(
    metadata: &Map<String, Value>,
    declared_variables: &BTreeSet<String>,
    unit_id: &str,
    path: &str,
    line: u32,
    project: &ProjectManifest,
) -> Result<(Node, Vec<Edge>), Diagnostic>;

fn declared_variables(text: &str) -> BTreeSet<String>;
```

Use a line-anchored ASL declaration recognizer equivalent to
`^\s*var\s+(_[A-Za-z][A-Za-z0-9_]*)\s*:`. Reject missing/additional fields,
invalid `PTO-STATE-*` IDs, invalid classification segments, unsupported scope,
wrong owner, empty members, duplicate members, members not declared in the
same file, and duplicate members across state records in the same owner.

- [ ] **Step 4: Add negative state cases and verify diagnostics**

`reject-state.asl` must cover:

```asl
// PTO-UNIT: {"id":"PTO-ARCH-STATE-OWNER","surface":"arch","classification":["state","owner"],"depends_on":[]}
// PTO-STATE: {"id":"PTO-STATE-ARCH-BAD-SCOPE","classification":["arch","bad"],"scope":"thread","owner":"PTO-ARCH-STATE-OWNER","members":["_Known"],"depends_on":[]}
// PTO-STATE: {"id":"PTO-STATE-ARCH-BAD-OWNER","classification":["arch","owner"],"scope":"core","owner":"PTO-ARCH-OTHER","members":["_Known"],"depends_on":[]}
// PTO-STATE: {"id":"PTO-STATE-ARCH-BAD-MEMBER","classification":["arch","member"],"scope":"core","owner":"PTO-ARCH-STATE-OWNER","members":["_Missing"],"depends_on":[]}
var _Known : bits(32);
```

Assert `NDF-ASL-008` for scope/schema failures and `NDF-ASL-009` for
owner/member failures.

- [ ] **Step 5: Review and update the golden output**

Run:

```bash
cargo test -p ndf-compiler --test pto_asl --locked
```

Expected before golden update: FAIL on the frozen oracle. Review, update the
golden JSON, rerun, and require PASS.

- [ ] **Step 6: Commit the state-node parser**

```bash
git add crates/ndf-compiler/src/adapters/pto_asl.rs crates/ndf-compiler/tests/pto_asl.rs tests/fixtures/pto-asl tests/golden/pto-asl.json
git commit -m "feat(pto): index architecture state families"
```

---

### Task 3: Validate the complete PTO unit/state graph

**Repository:** `PTO-ISA/normative_language`

**Files:**

- Add: `crates/ndf-compiler/src/adapters/pto_graph.rs`
- Modify: `crates/ndf-compiler/src/adapters/mod.rs`
- Modify: `crates/ndf-compiler/tests/pto_asl.rs`
- Add: `tests/fixtures/pto-asl-project/ndf.yaml`
- Add: `tests/fixtures/pto-asl-project/{owner,dependency,cycle-a,cycle-b}.asl`

**Interfaces:**

- Produces `pub(crate) fn validate_pto_graph(graph: Graph) -> Graph`.
- Resolves duplicate IDs, unit/state dependencies, the synthetic decoder node, state-owner edges, and cycles after all files are parsed.
- Preserves deterministic node, edge, and diagnostic ordering.

- [ ] **Step 1: Write failing project-level validation tests**

Add tests that create temporary complete projects and assert:

```rust
assert_eq!(diagnostic_codes(&unknown_unit), ["NDF-ASL-007"]);
assert_eq!(diagnostic_codes(&unit_cycle), ["NDF-ASL-007"]);
assert_eq!(diagnostic_codes(&unknown_state), ["NDF-ASL-009"]);
assert_eq!(diagnostic_codes(&state_cycle), ["NDF-ASL-009"]);
assert!(valid_graph.diagnostics.is_empty());
```

Also parse the same files in reversed lexical names and require byte-identical
serialized graph output.

- [ ] **Step 2: Run and verify RED**

Run:

```bash
cargo test -p ndf-compiler --test pto_asl complete_pto_graph --locked
```

Expected: FAIL because `parse_project` has no PTO-wide dependency validation.

- [ ] **Step 3: Implement graph validation**

Create `pto_graph.rs` with:

```rust
pub(crate) fn validate_pto_graph(graph: Graph) -> Graph;

fn cycle_nodes(
    nodes: &BTreeSet<NodeId>,
    edges: &BTreeMap<NodeId, BTreeSet<NodeId>>,
) -> BTreeSet<NodeId>;
```

Use a deterministic Kahn traversal. Emit one diagnostic per invalid source
node, not one per traversal path. Preserve all original nodes/edges and append
diagnostics through `GraphBuilder`.

Update `parse_project` to call `validate_pto_graph` only when
`manifest.project == "pto-spec"` or when at least one node has a PTO
`pto_entity` attribute. Do not apply PTO policy to unrelated adapters.

- [ ] **Step 4: Run compiler tests**

```bash
cargo test -p ndf-compiler --test pto_asl --locked
cargo test -p ndf-compiler --test dependencies --locked
```

Expected: PASS.

- [ ] **Step 5: Commit project validation**

```bash
git add crates/ndf-compiler/src/adapters crates/ndf-compiler/tests/pto_asl.rs tests/fixtures/pto-asl-project
git commit -m "feat(pto): validate ASL unit and state graphs"
```

---

### Task 4: Expose state queries and state-aware release impact

**Repository:** `PTO-ISA/normative_language`

**Files:**

- Modify: `crates/ndf-cli/src/args.rs`
- Modify: `crates/ndf-cli/src/run.rs`
- Modify: `crates/ndf-cli/tests/commands.rs`
- Modify: `crates/ndf-compiler/src/query.rs`
- Modify: `crates/ndf-compiler/src/pto_impact.rs`
- Modify: `crates/ndf-compiler/tests/query.rs`
- Modify: `crates/ndf-compiler/tests/pto_release_impact.rs`
- Modify: `tests/golden/query-report.json`
- Modify: `tests/fixtures/pto-impact/*.ndf.json`

**Interfaces:**

- Adds CLI `ndf query --index <sqlite> --expression <query> --format json` using existing `query_nodes` semantics.
- Recognizes `attributes.<name>=<value>` query clauses, including `attributes.pto_entity=state`.
- Classifies state scope/member changes as architectural-state breaking changes.
- Classifies state owner/source moves as review-required.
- Classifies additive state references in `PTO-ARCH-STATE-CLOSURE-001` as review-required.

- [ ] **Step 1: Write failing query CLI tests**

Add a command test equivalent to:

```rust
let output = ndf().args([
    "query", "--index", index(),
    "--expression", "kind=architecture and attributes.pto_entity=state",
    "--format", "json",
]).output().unwrap();
assert!(output.status.success());
assert_eq!(json(output)["command"], "query");
```

- [ ] **Step 2: Run and verify query RED**

```bash
cargo test -p ndf-cli --test commands query --locked
```

Expected: FAIL because Clap has no `query` command.

- [ ] **Step 3: Implement the CLI and attribute filtering**

Add:

```rust
Command::Query {
    #[arg(long, default_value = "graph.sqlite")]
    index: PathBuf,
    #[arg(long)]
    expression: String,
    #[arg(long, value_enum, default_value_t = JsonFormat::Json)]
    format: JsonFormat,
}
```

Extend `query_nodes` to accept `attributes.<key>` and compare against the
decoded node attribute map. Keep unsupported top-level fields fail-closed.

- [ ] **Step 4: Write failing state-impact cases**

Add fixtures and assertions for:

```rust
assert_eq!(scope_change.compatibility, Compatibility::Breaking);
assert_eq!(member_change.compatibility, Compatibility::Breaking);
assert_eq!(owner_move.compatibility, Compatibility::ReviewRequired);
assert_eq!(closure_expansion.compatibility, Compatibility::ReviewRequired);
```

- [ ] **Step 5: Run and verify impact RED**

```bash
cargo test -p ndf-compiler --test pto_release_impact state_ --locked
```

Expected: at least the owner-move and closure-expansion cases fail under the
current generic node comparison.

- [ ] **Step 6: Implement state-aware classification**

Add helpers with these contracts:

```rust
fn is_pto_state(node: &NodeRecord) -> bool;
fn state_semantics_equal(left: &NodeRecord, right: &NodeRecord) -> bool;
fn state_owner_only_changed(left: &NodeRecord, right: &NodeRecord) -> bool;
fn closure_only_added_state_references(
    before: &NdfIndex,
    after: &NdfIndex,
    uri: &str,
) -> Result<bool, CompilerError>;
```

State semantic fields are `scope`, `members`, `classification`, kind, status,
and body. Owner/source-only changes produce `Moved`. The closure helper must
require unchanged non-reference text, no removed outgoing reference, and only
new `references` targets whose local IDs begin `PTO-STATE-`.

- [ ] **Step 7: Run focused and regression tests**

```bash
cargo test -p ndf-compiler --test query --test pto_release_impact --locked
cargo test -p ndf-cli --test commands --locked
```

Expected: PASS.

- [ ] **Step 8: Commit query and impact support**

```bash
git add crates/ndf-cli crates/ndf-compiler/src/query.rs crates/ndf-compiler/src/pto_impact.rs crates/ndf-compiler/tests tests/golden/query-report.json tests/fixtures/pto-impact
git commit -m "feat(pto): query and classify state graph changes"
```

---

### Task 5: Document, verify, push, and open the NDF compiler PR

**Repository:** `PTO-ISA/normative_language`

**Files:**

- Modify: `docs/adapters/pto-asl.md`
- Modify: `normative_language.md`
- Modify: `normative_language_cn.md`
- Modify: `CHANGELOG.md`
- Verify: all files changed in Tasks 1-4

**Interfaces:**

- Documents exact `PTO-UNIT`, `PTO-STATE`, synthetic decoder, query, diagnostic, and impact contracts.
- Produces one reviewable GitHub PR whose merge commit becomes the only legal PTO-SPEC compiler pin.

- [ ] **Step 1: Update English and Chinese normative descriptions together**

Add the four PTO identity classes, exact state metadata fields, scope
vocabulary, and authority boundary to both language files. Keep examples
semantically identical across translations. Update `docs/adapters/pto-asl.md`
with diagnostics `NDF-ASL-006` through `NDF-ASL-009` and the exact query form.

- [ ] **Step 2: Run the complete NDF gate**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --locked
cargo build --workspace --release --locked
scripts/ndf check --root tests/fixtures/markdown/project --format json
NDF_RUN_SCALE_TESTS=1 cargo test -p ndf-compiler --test scale --release --locked
scripts/check-release-performance
git diff --check
```

Expected: every command exits 0.

- [ ] **Step 3: Commit documentation**

```bash
git add docs/adapters/pto-asl.md normative_language.md normative_language_cn.md CHANGELOG.md
git commit -m "docs(pto): define ASL state graph contract"
```

- [ ] **Step 4: Push and create the compiler PR**

```bash
git push -u origin codex/pto-asl-state-graph
gh pr create --repo PTO-ISA/normative_language \
  --base main --head codex/pto-asl-state-graph \
  --title "feat: index PTO ASL unit and state graphs" \
  --body-file /tmp/pto-asl-state-graph-pr.md
```

The PR body must link the design document, list diagnostic/CLI changes, state
that ASL semantics are unchanged, and include fresh gate output.

- [ ] **Step 5: Merge checkpoint**

Record the merged 40-character commit from `PTO-ISA/normative_language/main`.
Do not begin Task 6 until that merge exists.

---

### Task 6: Pin the merged compiler and add PTO-SPEC parity gates

**Repository:** `PTO-ISA/pto-spec`

**Files:**

- Add or modify: `.gitmodules`
- Add gitlink: `tools/ndf`
- Modify: `ndf.yaml`
- Modify: `ndf.lock`
- Add: `tests/scripts/test_ndf_state_graph_parity.py`
- Modify: `tests/scripts/test_ndf.py`
- Modify: `Makefile`
- Modify: `.github/workflows/asl.yml`
- Modify: `scripts/check-pr`

**Interfaces:**

- Consumes the merged NDF compiler commit from Task 5.
- Keeps `./scripts/check-ndf` as the PTO-local gate while adding full Rust graph parity.
- Records `ndf` as a tool-only dependency with `graph: false`.

- [ ] **Step 1: Add failing pin/parity tests**

Assert that the `tools/ndf` gitlink, `ndf.lock.dependencies.ndf.revision`, and
the checked-out compiler HEAD equal the exact merged commit. Invoke the built
compiler and require the exported graph to contain:

```python
assert counts["unit"] == 788
assert counts["instruction"] == 651
assert counts["synthetic-unit"] == 1
assert counts["state"] == 11
```

The unit/instruction counts are the current baseline; if upstream main changes
before implementation, update both the assertions and the PR body from a fresh
`load_units()` inventory in the same commit.

- [ ] **Step 2: Run and verify RED**

```bash
python3 -m unittest tests.scripts.test_ndf_state_graph_parity -v
```

Expected: FAIL because `tools/ndf` and state nodes are absent.

- [ ] **Step 3: Add the exact compiler submodule**

```bash
git submodule add https://github.com/PTO-ISA/normative_language.git tools/ndf
git -C tools/ndf fetch origin main
git -C tools/ndf checkout "$(git -C tools/ndf rev-parse origin/main)"
```

Before continuing, require `git -C tools/ndf rev-parse HEAD` to equal the exact
40-character merge commit recorded at the Task 5 checkpoint. Configure:

```yaml
dependencies:
  ndf:
    path: tools/ndf
    graph: false
```

and record URI, revision, and path under `ndf.lock.dependencies.ndf`.

- [ ] **Step 4: Wire the Rust parity check into PR CI**

Build with:

```bash
cargo build --manifest-path tools/ndf/Cargo.toml --locked --release -p ndf-cli
```

Add the parity test after `check-ndf` in `make pr-check`, `scripts/check-pr`,
and `.github/workflows/asl.yml`. Ensure checkout initializes submodules.

- [ ] **Step 5: Run focused parity tests**

```bash
python3 -m unittest tests.scripts.test_ndf_state_graph_parity tests.scripts.test_ndf -v
```

Expected at this checkpoint: compiler pin checks pass; the state count remains
RED until Task 7 authors the catalog.

- [ ] **Step 6: Commit the compiler pin separately**

```bash
git add .gitmodules tools/ndf ndf.yaml ndf.lock Makefile .github/workflows/asl.yml scripts/check-pr tests/scripts
git commit -m "build(ndf): pin PTO state graph compiler"
```

---

### Task 7: Rewrite PTO-SPEC PR 66 around the complete state catalog

**Repository:** `PTO-ISA/pto-spec`

**Files:**

- Modify: `asl/arch/programming-model/execution-context.asl`
- Modify: `asl/tile/model/state/local-registers.asl`
- Modify: `asl/block/model/state/control-state.asl`
- Modify: `asl/arch/overview/architecture.asl`
- Modify: `asl/arch/features/shared-tile-state.asl`
- Modify: `asl/arch/data-types/integer.asl`
- Modify: `scripts/ndf.py`
- Modify: `scripts/asl_units.py`
- Modify: `tests/scripts/test_ndf.py`
- Modify: `tests/scripts/test_ndf_state_graph_parity.py`
- Regenerate: corresponding `docs/` ASL mirrors
- Regenerate: `tests/asl/**/PTO-AVS-*-STATIC-INVARIANT-001.asl`
- Regenerate: `spec/evidence/*.json`
- Regenerate: `spec/release-manifest.json`

**Interfaces:**

- Produces the exact 11 state identities from the approved design.
- Migrates `PTO-REQ-STATE-001`, `PTO-REQ-TILE-001`, `PTO-REQ-SHARED-TILE-001`, and `PTO-REQ-BUNDLE-STATE-001` into explicit NDF regions.
- Makes `PTO-ARCH-STATE-CLOSURE-001` reference every state node.
- Removes `PTO-ARCH-SHARED-TILE-STATE-001` and all DavinciOO-specific anchor test names.

- [ ] **Step 1: Write failing PTO-local state metadata tests**

Extend `tests/scripts/test_ndf.py` with exact expected IDs and member ownership:

```python
self.assertEqual(
    set(state_index),
    {
        "PTO-STATE-ARCH-GPR",
        "PTO-STATE-ARCH-TEMPORARY-QUEUES",
        "PTO-STATE-ARCH-PROGRAM-CONTROL",
        "PTO-STATE-ARCH-FAULT",
        "PTO-STATE-ARCH-MEMORY",
        "PTO-STATE-ARCH-MAINTENANCE",
        "PTO-STATE-ARCH-SYSTEM-REGISTERS",
        "PTO-STATE-ARCH-TRAP-CONTEXT",
        "PTO-STATE-TILE-LOCAL",
        "PTO-STATE-TILE-SHARED",
        "PTO-STATE-BLOCK-CONTROL",
    },
)
```

Require every member to match a same-file ASL `var` declaration and occur in
exactly one state family.

- [ ] **Step 2: Run and verify RED**

```bash
python3 -m unittest tests.scripts.test_ndf tests.scripts.test_ndf_state_graph_parity -v
```

Expected: FAIL because no `PTO-STATE` records exist and the obsolete Shared
Tile anchor still exists.

- [ ] **Step 3: Add PTO-local metadata parsing and validation**

Add a frozen dataclass and loader in `scripts/ndf.py` with exact public
signatures `parse_state_records(text: str, source: Path, unit_id: str) ->
tuple[PtoState, ...]` and `state_index(root: Path) -> dict[str, PtoState]`:

```python
@dataclass(frozen=True)
class PtoState:
    state_id: str
    classification: tuple[str, ...]
    scope: str
    owner: str
    members: tuple[str, ...]
    depends_on: tuple[str, ...]
    source: Path
    line: int
```

Mirror the Rust validation contract exactly; do not create a second metadata
dialect. Add state IDs to `known_ids` for NDF references.

- [ ] **Step 4: Author the 11 state records beside their declarations**

Use exact ASL member identifiers. `PTO-STATE-ARCH-TEMPORARY-QUEUES` has
`scope=bundle`; split system registers from ACR trap contexts; keep local Tile
and Shared Tile separate even though their storage variables share one owner
file. Assign every declared member selected by the approved catalog exactly
once.

- [ ] **Step 5: Migrate requirement comments and rewrite closure**

Move each of the four canonical `PTO-REQ-*` definitions into one
`NDF-BEGIN`/`NDF-END` region at the semantic owner. Convert duplicate comment
definitions such as those in `integer.asl` into non-owning references. Rewrite
`PTO-ARCH-STATE-CLOSURE-001` so its body references all 11 state IDs.

Delete the complete `PTO-ARCH-SHARED-TILE-STATE-001` region from
`shared-tile-state.asl`; retain the ASL unit and its dependency unchanged.

- [ ] **Step 6: Run focused checks and verify GREEN**

```bash
python3 -m unittest tests.scripts.test_ndf tests.scripts.test_ndf_state_graph_parity -v
./scripts/check-ndf
./scripts/check-asl-layout
```

Expected: PASS with 788 unit, 651 instruction, one synthetic-unit, and 11 state
nodes at the recorded baseline.

- [ ] **Step 7: Regenerate owned projections**

```bash
python3 scripts/instruction_docs.py
python3 scripts/generate-mnemonic-avs.py
make release-prepare
```

Review generated diffs. Static `assert TRUE` files may prove only independent
compilation; PR text must not describe them as state-transition conformance.

- [ ] **Step 8: Commit the generic state graph separately**

```bash
git add asl docs scripts tests spec
git commit -m "feat(ndf): publish PTO architecture state graph"
```

- [ ] **Step 9: Rebase the release-event commit without changing its contract**

Keep the existing release-event schema implementation as a distinct commit
after the state-graph commit. Resolve generated evidence hashes mechanically;
do not mix new architecture metadata into release payload fields.

- [ ] **Step 10: Run the full PTO gates**

```bash
make pr-check
make repo-check
make release-prepare
git diff --check
```

Expected: PASS. If the prepared ASLRef cache is already exact, also run
`make release-verify RELEASE_COMMIT="$(git rev-parse HEAD)"`; otherwise report
that exact-head hosted release verification remains required.

- [ ] **Step 11: Rewrite and push PR 66**

Force-with-lease is permitted only for this explicitly requested PR rewrite:

```bash
git push --force-with-lease origin codex/pto-release-dispatch-a
gh pr edit 66 --repo PTO-ISA/pto-spec \
  --title "feat: publish PTO ASL state graph and release event schema" \
  --body-file /tmp/pto-spec-pr66-state-graph.md
```

The new body must name the 11 state IDs, removed Shared Tile-only anchor,
compiler merge commit, unchanged ASL behavior, release-event isolation, and
fresh verification results. Keep auto-merge disabled.

- [ ] **Step 12: Merge checkpoint**

Record the merged 40-character PTO-SPEC commit. Do not advance DavinciOO's
`isa` gitlink to an unmerged PR head.

---

### Task 8: Update DavinciOO state conformance and compatibility evidence

**Repository:** `hengliao1972/DavinciOO`

**Files:**

- Modify gitlink: `toolchains/ndf`
- Modify gitlink: `isa`
- Modify: `ndf.lock`
- Modify: `docs/specification/davincioo/ndf/30-interfaces/isa.md`
- Modify: `docs/specification/davincioo/ndf/10-architecture/resources.md`
- Modify: `docs/specification/davincioo/ndf/20-behavior/rename.md`
- Modify: `docs/specification/davincioo/ndf/20-behavior/rob.md`
- Modify: `docs/specification/davincioo/ndf/20-behavior/memory.md`
- Modify: `docs/specification/davincioo/ndf/60-warp/predicate.md`
- Modify: `docs/specification/davincioo/ndf/60-warp/warp-model.md`
- Modify: `srcs/tests/unit/test_pto_conformance_baseline.py`
- Modify: `srcs/tests/unit/test_pto_spec_contract.py`
- Regenerate: compatibility evidence under `docs/specification/davincioo/compatibility/`

**Interfaces:**

- Consumes only the merged NDF and PTO-SPEC commits from Tasks 5 and 7.
- Preserves DavinciOO local clause IDs and normative bodies; changes only cross-project metadata unless a missing implementation contract is discovered and separately reviewed.
- Produces state-specific `conforms-to` mappings plus the broad closure mapping.

- [ ] **Step 1: Write failing expected-mapping tests**

Require:

```python
EXPECTED_STATE_TARGETS = {
    "ISA-STATE-001": {"ndf://pto-spec/PTO-ARCH-STATE-CLOSURE-001"},
    "ISA-STGB-001": {"ndf://pto-spec/PTO-STATE-TILE-SHARED"},
    "RNM-001": {"ndf://pto-spec/PTO-STATE-ARCH-GPR"},
    "RES-TILE-001": {
        "ndf://pto-spec/PTO-STATE-TILE-LOCAL",
        "ndf://pto-spec/PTO-STATE-TILE-SHARED",
    },
    "ROB-ENTRY-001": {
        "ndf://pto-spec/PTO-STATE-ARCH-PROGRAM-CONTROL",
        "ndf://pto-spec/PTO-STATE-ARCH-FAULT",
    },
    "MEM-TLSU-002": {"ndf://pto-spec/PTO-STATE-ARCH-MEMORY"},
    "PRED-001": {"ndf://pto-spec/PTO-STATE-ARCH-PROGRAM-CONTROL"},
    "WARP-PC-001": {"ndf://pto-spec/PTO-STATE-ARCH-PROGRAM-CONTROL"},
}
```

These are the initial direct ownership mappings. Temporary queues,
maintenance, system-register/trap-context, and block-control state remain
covered only through `ISA-STATE-001` until DavinciOO has a dedicated local
clause that owns each contract. The test must fail if a PTO unit URI such as
`ndf://pto-spec/PTO-ARCH-*` is used as a state conformance target unless it is
the explicit closure contract.

- [ ] **Step 2: Run and verify RED**

```bash
pytest srcs/tests/unit/test_pto_conformance_baseline.py srcs/tests/unit/test_pto_spec_contract.py -q
```

Expected: FAIL because pins and targets still describe the old PR 66 anchors.

- [ ] **Step 3: Advance exact merged pins**

Checkout the recorded merged commits in both submodules and update the exact
40-character revisions in `ndf.lock`. Verify:

```bash
toolchains/ndf/target/release/ndf report dependencies --root . --format json
```

Expected: exact, clean `ndf` and `pto-spec` dependencies.

- [ ] **Step 4: Add state-specific conformance edges**

Keep `ISA-STATE-001 -> PTO-ARCH-STATE-CLOSURE-001`. Replace
`ISA-STGB-001 -> PTO-ARCH-SHARED-TILE-STATE-001` with
`ISA-STGB-001 -> PTO-STATE-TILE-SHARED`. Add the remaining state targets to the
existing microarchitecture clauses that own those mechanisms. Do not conform
to `PTO-UNIT` source nodes.

- [ ] **Step 5: Rebuild the index and compatibility report**

```bash
python3 toolchains/scripts/build_ndf_index.py --check
python3 toolchains/scripts/render_pto_compatibility.py --check
```

Expected: zero new diagnostics and a reviewable state-graph compatibility
classification.

- [ ] **Step 6: Run DavinciOO gates**

```bash
pytest srcs/tests/unit -q
python3 toolchains/scripts/build_ndf_index.py --check
pre-commit run --all-files
git diff --check
```

Expected: PASS.

- [ ] **Step 7: Commit, push, and update the DavinciOO PR**

```bash
git add toolchains/ndf isa ndf.lock docs/specification srcs/tests
git commit -m "docs(ndf): conform DavinciOO to PTO state graph"
git push origin codex/ndf-docs-refactor
```

Update the existing PR body with exact merged pins, state mappings, impact
classification, and verification evidence. Do not enable automatic merge.

---

## Final Cross-Repository Verification

- [ ] The NDF compiler branch contains no generated database or target artifact.
- [ ] PTO-SPEC PR 66 contains no `PTO-ARCH-SHARED-TILE-STATE-001` identity.
- [ ] PTO-SPEC reports exactly 11 state nodes and resolves every state/member/edge.
- [ ] DavinciOO contains no conformance edge to a PTO unit node.
- [ ] Every committed `ndf.lock` revision equals its exact clean gitlink HEAD.
- [ ] NDF impact distinguishes state semantics, state owner moves, closure expansion, and unrelated additions.
- [ ] All fresh gate outputs are copied into the corresponding PR bodies.
- [ ] Auto-merge remains disabled on all bring-up PRs.
