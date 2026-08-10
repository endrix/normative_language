# NDF Rust Compiler Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the merged NDF Python prototype with a standalone Rust compiler that preserves the accepted format, IR, diagnostics, SQLite, and CLI contracts before the first NDF release.

**Architecture:** A three-crate Cargo workspace separates canonical graph contracts (`ndf-core`), compiler and database behavior (`ndf-compiler`), and the process interface (`ndf-cli`). Python remains executable only while language-neutral golden fixtures and cross-language parity tests are built; it is deleted after Rust satisfies correctness, determinism, and scale gates.

**Tech Stack:** Rust 1.94.0, Cargo edition 2024, Clap 4.6.6, Serde 1.0.229, serde_json 1.0.151, serde_yaml_ng 0.10.0, Rusqlite 0.40.2 with bundled SQLite, SHA-2 0.11.0, thiserror 2.0.20, pulldown-cmark 0.13.4, globset 0.4.20, walkdir 2.5.0, tempfile 3.27.0, assert_cmd 2.2.2, predicates 3.1.4, GitHub Actions.

## Global Constraints

- Preserve NDF format `0.2`, IR `0.1`, machine-output schema `0.1`, SQLite schema `1`, and current diagnostic codes.
- The first `v0.1.0` tag points to the merged Rust release commit; do not tag or pin the Python prototype.
- Markdown, ASL, source, test, and evidence files remain normative inputs; SQLite remains disposable derived state.
- PTO-SPEC ASL remains authoritative for PTO architecture; DavinciOO NDF clauses remain authoritative for DavinciOO microarchitecture.
- Provide one standalone `ndf` binary. Downstream repositories must not install or import the NDF Python package.
- Commit `Cargo.lock` and pin Rust `1.94.0` in `rust-toolchain.toml`.
- Preserve command names, arguments, exit meanings, JSON envelopes, field names, ordering, source spans, and semantic hashes unless an explicit contract test approves a change.
- Use map/set graph construction, one SQLite transaction with prepared batches, SQL query filtering, and streaming JSON export.
- Do not add PyO3, supported Python bindings, writable database APIs, runtime-loaded Rust dynamic libraries, or a background service.
- Execute inline in the existing isolated worktree without subagents.
- Observe every Rust production test fail before implementing the behavior.

## File Map

```text
Cargo.toml                         # workspace and shared dependency versions
Cargo.lock                        # exact dependency graph
rust-toolchain.toml               # Rust 1.94.0 with rustfmt and clippy
crates/ndf-core/src/              # identity, model, graph builder, diagnostics, versions
crates/ndf-compiler/src/          # manifests, parsers, rules, adapters, SQLite, queries
crates/ndf-cli/src/               # Clap commands, JSON envelopes, exit behavior
schemas/                          # existing language-neutral JSON Schemas
sql/schema-v1.sql                 # language-neutral SQLite schema
tests/fixtures/                   # shared source fixtures
tests/golden/                     # normalized Python oracle results
tests/oracle/                     # temporary migration-only Python harness
tests/scale/                      # deterministic scale workload
```

---

### Task 1: Freeze Python behavior into language-neutral golden files

**Files:**
- Create: `tests/oracle/generate_golden.py`
- Create: `tests/oracle/normalize.py`
- Create: `tests/test_oracle_golden.py`
- Create: `tests/golden/{machine-version,identity,markdown-valid,markdown-invalid,pto-asl,index,query-report}.json`

**Interfaces:**
- Consumes: merged `src/ndf` and current `tests/fixtures`.
- Produces: `python3 tests/oracle/generate_golden.py --check`.
- Produces: sorted UTF-8 JSON with repository-relative POSIX paths, fixed revision `1111111111111111111111111111111111111111`, and one trailing newline.

- [ ] **Step 1: Write the failing consistency test**

```python
class OracleGoldenTest(unittest.TestCase):
    def test_committed_golden_matches_python(self) -> None:
        result = subprocess.run(
            [sys.executable, "tests/oracle/generate_golden.py", "--check"],
            cwd=Path(__file__).parents[1], text=True, capture_output=True,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
```

- [ ] **Step 2: Verify RED**

Run: `python3 -m unittest tests.test_oracle_golden -v`
Expected: FAIL because the generator is absent.

- [ ] **Step 3: Implement normalization and generation**

```python
def normalized_json(value: object) -> str:
    return json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
```

Generate golden output through public Python functions and `ndf.cli.main`. In
check mode, compare without writing and print every mismatched filename.

- [ ] **Step 4: Verify GREEN and commit**

Run: `python3 tests/oracle/generate_golden.py && python3 -m unittest tests.test_oracle_golden -v`
Expected: PASS.

```bash
git add tests/oracle tests/golden tests/test_oracle_golden.py
git commit -m "test: freeze the Python NDF behavior"
```

---

### Task 2: Scaffold the Cargo workspace and version command

**Files:**
- Create: `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`
- Create: `crates/ndf-core/{Cargo.toml,src/lib.rs,src/version.rs}`
- Create: `crates/ndf-compiler/{Cargo.toml,src/lib.rs}`
- Create: `crates/ndf-cli/{Cargo.toml,src/main.rs,tests/machine_version.rs}`
- Modify: `.github/workflows/ci.yml`, `.gitignore`

**Interfaces:**
- Produces: binary `ndf` and `ndf_core::version` constants.
- Produces: `ndf --machine-version` equal to `tests/golden/machine-version.json`.

- [ ] **Step 1: Write the failing binary test**

```rust
#[test]
fn machine_version_matches_the_oracle() {
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/golden/machine-version.json"
    )).unwrap();
    let output = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("ndf"))
        .arg("--machine-version").output().unwrap();
    assert!(output.status.success());
    assert_eq!(serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(), expected);
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p ndf-cli --test machine_version`
Expected: FAIL because the workspace is absent.

- [ ] **Step 3: Implement the minimal workspace**

Use resolver `2`, edition `2024`, Rust `1.94`, and the exact versions in this
plan header. Enable `clap/derive`, `serde/derive`, and `rusqlite/bundled`.
Implement only the five version constants and `--machine-version`.

- [ ] **Step 4: Add Rust CI and verify GREEN**

Run: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace --locked && python3 -m unittest tests.test_oracle_golden -v`
Expected: PASS. Keep Python CI until parity is complete.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock rust-toolchain.toml crates .github/workflows/ci.yml .gitignore
git commit -m "feat: scaffold the Rust NDF compiler"
```

---

### Task 3: Implement canonical identities and constant-time graph building

**Files:**
- Create: `crates/ndf-core/src/{identity,model,graph}.rs`
- Create: `crates/ndf-core/tests/{identity,graph}.rs`
- Modify: `crates/ndf-core/src/lib.rs`

**Interfaces:**
- Produces: `NodeId::parse(value, current_project)`, `Display`, and `uri()`.
- Produces: Serde types `SourceSpan`, `Node`, `Edge`, `Diagnostic`, `ParseResult`, and `Graph`.
- Produces: `GraphBuilder::{add_node,add_edge,add_diagnostic,finish}` using `HashMap<NodeId, Node>`.

- [ ] **Step 1: Write failing identity and duplicate tests**

```rust
#[test]
fn duplicate_id_keeps_the_first_node_and_emits_ndf_id_001() {
    let mut builder = GraphBuilder::default();
    builder.add_node(node("REQ-001", 4));
    builder.add_node(node("REQ-001", 9));
    let graph = builder.finish();
    assert_eq!(graph.nodes.len(), 1);
    assert_eq!(graph.diagnostics[0].code, "NDF-ID-001");
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p ndf-core --test identity --test graph`
Expected: FAIL because the types are absent.

- [ ] **Step 3: Implement and sort only at freeze boundaries**

Validate project/local components, preserve explicit null JSON fields, and sort
nodes by URI, edges by `(source,type,target)`, and diagnostics by
`(path,line,code)` in `finish()`.

- [ ] **Step 4: Verify GREEN and commit**

Run: `cargo test -p ndf-core --locked && cargo clippy -p ndf-core --all-targets -- -D warnings`
Expected: PASS, including comparison with `tests/golden/identity.json`.

```bash
git add crates/ndf-core
git commit -m "feat: define the Rust NDF graph model"
```

---

### Task 4: Port manifests, locks, and Markdown parsing

**Files:**
- Create: `crates/ndf-compiler/src/{manifest,markdown}.rs`
- Create: `crates/ndf-compiler/tests/{manifest,markdown}.rs`
- Modify: `crates/ndf-compiler/src/lib.rs`

**Interfaces:**
- Produces: `ProjectManifest::load`, `ProjectLock::load`, and `parse_markdown`.
- Preserves: front matter, heading IDs, NDF comments, fenced-code exclusion,
  clause bodies, edges, source spans, and `NDF-MIG-001`.

- [ ] **Step 1: Write failing golden tests**

```rust
#[test]
fn valid_markdown_matches_the_oracle() {
    let manifest = ProjectManifest::load(&fixture("markdown/project/ndf.yaml")).unwrap();
    let parsed = parse_markdown(&fixture("markdown/project/docs/pipeline.md"), &manifest).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), golden("markdown-valid.json"));
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p ndf-compiler --test manifest --test markdown`
Expected: FAIL because the parser is absent.

- [ ] **Step 3: Implement strict parsing**

Use Serde YAML structs with unknown fields denied and 40-hex lock validation.
Use pulldown-cmark offset events for headings/fences and source slices for the
immediately following metadata comment. Map only the four documented legacy
fields and emit `NDF-MIG-001`; do not infer ambiguous mappings.

- [ ] **Step 4: Verify GREEN and commit**

Run: `cargo test -p ndf-compiler --test manifest --test markdown --locked`
Expected: PASS against valid and invalid golden JSON.

```bash
git add crates/ndf-compiler
git commit -m "feat: parse NDF projects in Rust"
```

---

### Task 5: Port generic rules and no-regression baselines

**Files:**
- Create: `crates/ndf-compiler/src/{rules,baseline}.rs`
- Create: `crates/ndf-compiler/tests/{rules,baseline}.rs`

**Interfaces:**
- Produces: `validate_graph`, `diagnostic_fingerprint`, and `compare_baseline`.
- Preserves: dangling-edge, prefix, refinement, lifecycle, owner, and baseline diagnostics.

- [ ] **Step 1: Write failing rule tests**

```rust
#[test]
fn dangling_mandatory_edge_is_an_error() {
    let diagnostics = validate_graph(&graph_with_missing_target(), &ValidationPolicy::default());
    assert_eq!(diagnostics[0].code, "NDF-REF-001");
    assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p ndf-compiler --test rules --test baseline`
Expected: FAIL because rule APIs are absent.

- [ ] **Step 3: Implement deterministic rules and fingerprints**

Port one Python rule per focused test. Fingerprint the normalized tuple
`(code,subject,relative-path,line,message-class)` and exclude absolute roots.

- [ ] **Step 4: Verify GREEN and commit**

Run: `cargo test -p ndf-compiler --test rules --test baseline --locked`
Expected: PASS and ordered diagnostics equal `markdown-invalid.json`.

```bash
git add crates/ndf-compiler
git commit -m "feat: enforce NDF rules in Rust"
```

---

### Task 6: Build deterministic SQLite indexes atomically

**Files:**
- Move: `src/ndf/sql/schema-v1.sql` to `sql/schema-v1.sql`
- Create: `crates/ndf-compiler/src/index.rs`
- Create: `crates/ndf-compiler/tests/{index,index_determinism}.rs`

**Interfaces:**
- Produces: `BuildProvenance`, `IndexBuild`, `NdfIndex::open`, and `build_index`.
- Preserves: schema `1`, foreign keys, provenance, semantic hashes, and semantic rows.

- [ ] **Step 1: Write failing determinism and atomicity tests**

```rust
#[test]
fn two_builds_have_equal_hashes_and_semantic_rows() {
    let first = build_fixture_index("first.sqlite");
    let second = build_fixture_index("second.sqlite");
    assert_eq!(first.semantic_hash, second.semantic_hash);
    assert_eq!(dump_semantic_rows(&first.output), dump_semantic_rows(&second.output));
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p ndf-compiler --test index --test index_determinism`
Expected: FAIL because the index API is absent.

- [ ] **Step 3: Implement canonical hashing and batch insertion**

Stream canonical JSON fragments into SHA-256. Build a temporary database beside
the destination, apply `sql/schema-v1.sql`, insert through prepared statements
inside one transaction, close it, then atomically persist it. Refuse output under
a normative root and preserve the prior index on failure.

- [ ] **Step 4: Verify GREEN and commit**

Run: `cargo test -p ndf-compiler --test index --test index_determinism --locked`
Expected: PASS and semantic output equal `tests/golden/index.json`.

```bash
git add sql crates/ndf-compiler src/ndf/sql/schema-v1.sql
git commit -m "feat: materialize NDF indexes in Rust"
```

---

### Task 7: Port SQL-backed graph workflows

**Files:**
- Create: `crates/ndf-compiler/src/{query,report,diff,export}.rs`
- Create: `crates/ndf-compiler/tests/{query,workflows}.rs`

**Interfaces:**
- Produces: URI resolution, node lookup, typed in/out edges, trace, coverage,
  semantic diff, and buffered NDF JSON export.

- [ ] **Step 1: Write failing workflow parity test**

```rust
#[test]
fn query_and_report_match_the_oracle() {
    let index = fixture_index();
    let actual = json!({
        "show": index.get_node("ndf://davincioo/PIPE-ISSUE-001").unwrap(),
        "trace": trace(&index, "PIPE-ISSUE-010", &["refines"]).unwrap(),
        "coverage": coverage(&index, &CoveragePolicy::default()).unwrap(),
    });
    assert_eq!(actual, golden("query-report.json"));
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p ndf-compiler --test query --test workflows`
Expected: FAIL because workflow modules are absent.

- [ ] **Step 3: Implement SQL filtering and streaming export**

Filter URI, direction, type, status, kind, domain, and coverage candidates in
SQL. Preserve cycle-safe deterministic trace order. Stream ordered export records
through a buffered Serde serializer without building a second graph value.

- [ ] **Step 4: Verify GREEN and commit**

Run: `cargo test -p ndf-compiler --test query --test workflows --locked`
Expected: PASS.

```bash
git add crates/ndf-compiler
git commit -m "feat: expose Rust NDF graph workflows"
```

---

### Task 8: Port PTO ASL, repository, evidence, and dependency adapters

**Files:**
- Create: `crates/ndf-compiler/src/adapters/{mod,pto_asl,repository,evidence}.rs`
- Create: `crates/ndf-compiler/src/dependencies.rs`
- Create: `crates/ndf-compiler/tests/{pto_asl,repository_evidence,dependencies}.rs`

**Interfaces:**
- Produces: internal `Adapter` trait with `parse(&self, path, manifest) -> Result<ParseResult, CompilerError>` and a manifest-selected static registry.
- Produces: `parse_pto_asl`, `index_repository`, `load_evidence`, and `resolve_dependencies`.
- Requires: adapters inspect checked-out state but never clone, fetch, checkout, reset, or write repositories.

- [ ] **Step 1: Write failing PTO and dependency tests**

```rust
#[test]
fn pto_asl_matches_the_oracle() {
    assert_eq!(serde_json::to_value(parse_pto_fixture()).unwrap(), golden("pto-asl.json"));
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p ndf-compiler --test pto_asl --test repository_evidence --test dependencies`
Expected: FAIL because adapters are absent.

- [ ] **Step 3: Port explicit mappings and pure resolution**

Implement the internal `Adapter` trait and statically register the built-in
Markdown and PTO ASL adapters. Preserve ASL authority metadata and current clause identity/body/reference rules.
Derive source/test IDs from project, path, revision, and content identity. Validate
evidence against the existing schema. Permit only read-only Git inspection.

- [ ] **Step 4: Verify GREEN and commit**

Run: `cargo test -p ndf-compiler --test pto_asl --test repository_evidence --test dependencies --locked`
Expected: PASS.

```bash
git add crates/ndf-compiler
git commit -m "feat: port NDF adapters to Rust"
```

---

### Task 9: Complete the Rust CLI, ID allocation, and safe fixes

**Files:**
- Create: `crates/ndf-compiler/src/{id_allocator,fix}.rs`
- Create: `crates/ndf-cli/src/{args,envelope,run}.rs`
- Modify: `crates/ndf-cli/src/main.rs`
- Create: `crates/ndf-cli/tests/{commands,errors}.rs`

**Interfaces:**
- Produces: `build`, `check`, `show`, `trace`, `report coverage`,
  `report dependencies`, `diff`, `export`, `id allocate`, and `fix`.
- Produces: `{schema_version,command,ok,data,diagnostics}`; exit `0` for success,
  `1` for source/policy failure, and `2` for invalid invocation.

- [ ] **Step 1: Write one failing end-to-end test per command**

```rust
#[test]
fn check_emits_the_versioned_envelope() {
    let output = ndf().args(["check", "--root", fixture_root(), "--format", "json"])
        .output().unwrap();
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schema_version"], "0.1");
    assert_eq!(value["command"], "check");
    assert_eq!(value["ok"], true);
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p ndf-cli --test commands --test errors`
Expected: FAIL because only `--machine-version` exists.

- [ ] **Step 3: Implement command dispatch and structured errors**

Keep Clap types in `args.rs`, JSON construction in `envelope.rs`, and compiler
calls in `run.rs`. Allocate the lowest unused suffix. Safe fix may normalize
metadata or generated navigation but must reject semantic choices.

- [ ] **Step 4: Verify GREEN and commit**

Run: `cargo test -p ndf-cli --locked && cargo clippy -p ndf-cli --all-targets -- -D warnings`
Expected: PASS for all commands, unknown/ambiguous IDs, invalid invocation, and fix refusal.

```bash
git add crates/ndf-cli crates/ndf-compiler
git commit -m "feat: complete the Rust NDF command surface"
```

---

### Task 10: Prove parity, determinism, and scale

**Files:**
- Create: `tests/oracle/compare_rust.py`, `tests/test_rust_parity.py`
- Create: `tests/scale/generate.rs`, `crates/ndf-compiler/tests/scale.rs`
- Create: `scripts/check-release-performance`
- Create: `docs/performance/reference-machine.md`

**Interfaces:**
- Produces: complete Python-versus-Rust golden parity.
- Produces: opt-in 100,000-node/1,000,000-edge release test.
- Enforces: elapsed time below 10 seconds and peak RSS below 536,870,912 bytes on the documented reference machine.

- [ ] **Step 1: Write the failing parity test**

```python
class RustParityTest(unittest.TestCase):
    def test_rust_matches_every_golden_contract(self) -> None:
        result = subprocess.run([sys.executable, "tests/oracle/compare_rust.py"],
                                cwd=REPOSITORY, text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
```

- [ ] **Step 2: Verify RED**

Run: `python3 -m unittest tests.test_rust_parity -v`
Expected: FAIL because the comparison harness is absent or reports named parity gaps.

- [ ] **Step 3: Close every parity gap**

Build `target/release/ndf`. Compare parsed JSON, exit codes, semantic hashes, and
semantic SQLite rows. Normalize only temporary roots and path separators; never
normalize diagnostics, spans, messages, order, hashes, or data fields.

- [ ] **Step 4: Add deterministic scale evidence**

Generate the scale graph from numeric sequences. Build it twice. Measure with
`/usr/bin/time -l` on macOS or `/usr/bin/time -v` on Linux. Reject time
`>= 10.0` seconds or RSS `>= 536870912` bytes.

- [ ] **Step 5: Verify and commit**

Run: `cargo test --workspace --locked && python3 -m unittest tests.test_oracle_golden tests.test_rust_parity -v && NDF_RUN_SCALE_TESTS=1 cargo test -p ndf-compiler --test scale --release --locked && scripts/check-release-performance`
Expected: PASS under both scale thresholds.

```bash
git add tests scripts docs/performance crates/ndf-compiler/tests/scale.rs
git commit -m "test: prove Rust NDF parity and scale"
```

---

### Task 11: Remove Python and prepare the Rust-only release

**Files:**
- Delete: `pyproject.toml`, `requirements-lock.txt`, `src/ndf/`, tracked `src/ndf.egg-info/`
- Delete: `tests/index_test_support.py`
- Delete: `tests/test_baseline.py`, `tests/test_cli.py`, `tests/test_dependencies.py`
- Delete: `tests/test_diff.py`, `tests/test_evidence_adapter.py`, `tests/test_export.py`
- Delete: `tests/test_fix.py`, `tests/test_id_allocator.py`, `tests/test_identity.py`
- Delete: `tests/test_index.py`, `tests/test_index_scale.py`, `tests/test_manifest.py`
- Delete: `tests/test_markdown_parser.py`, `tests/test_model.py`, `tests/test_pto_asl_adapter.py`
- Delete: `tests/test_query.py`, `tests/test_report.py`, `tests/test_repository_adapter.py`
- Delete: `tests/test_rules.py`, `tests/test_spec_sync.py`
- Delete: `tests/oracle/generate_golden.py`, `tests/oracle/normalize.py`, `tests/oracle/compare_rust.py`
- Delete: `tests/test_oracle_golden.py`, `tests/test_rust_parity.py`, `tests/__init__.py`
- Create: `scripts/ndf`, `crates/ndf-cli/tests/release_contract.rs`
- Create: `crates/ndf-core/tests/spec_sync.rs`
- Modify: `.github/workflows/ci.yml`, `README.md`, `CHANGELOG.md`
- Modify: `docs/format/migration-v0.1-to-v0.2.md`
- Modify: `docs/superpowers/plans/2026-08-10-ndf-three-repository-refactor.md`

**Interfaces:**
- Produces: Rust-only source tree and `scripts/ndf` wrapper using `cargo run --locked --release -p ndf-cli -- "$@"`.
- Produces: three-repository plan using only the NDF binary JSON interface.

- [ ] **Step 1: Write the failing release-tree test**

```rust
#[test]
fn release_tree_has_no_python_ndf_runtime() {
    let root = repository_root();
    for forbidden in ["pyproject.toml", "requirements-lock.txt", "src/ndf"] {
        assert!(!root.join(forbidden).exists(), "{forbidden} must be removed");
    }
    assert!(root.join("Cargo.lock").is_file());
    assert!(root.join("scripts/ndf").is_file());
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p ndf-cli --test release_contract --locked`
Expected: FAIL on the existing Python runtime files.

- [ ] **Step 3: Convert permanent parity assertions and delete Python**

Move every permanent golden assertion into Rust integration tests, then delete
the Python package and migration harness. Remove NDF pip/PyYAML CI. Keep golden
JSON fixtures and performance evidence.

- [ ] **Step 4: Update documentation and downstream commands**

Document Rust 1.94.0, Cargo use, crate boundaries, and lack of Python bindings.
Change PTO-SPEC and DavinciOO NDF setup from pip/imports to
`cargo build --locked --release` plus CLI JSON subprocesses.

- [ ] **Step 5: Run the release-candidate gate**

Run: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace --all-features --locked && cargo build --workspace --release --locked && scripts/ndf check --root tests/fixtures/markdown/project --format json && NDF_RUN_SCALE_TESTS=1 cargo test -p ndf-compiler --test scale --release --locked && scripts/check-release-performance && git diff --check`
Expected: PASS with no database or cache artifacts.

- [ ] **Step 6: Commit, merge by PR, and tag the merged commit**

```bash
git add -A
git commit -m "feat: ship NDF as a Rust knowledge compiler"
```

Push and open a ready PR. After CI and merge, fetch `origin/main`, verify the
merge contains this commit, then create and push annotated `v0.1.0` on the exact
merged commit. Never tag the feature-branch commit.

---

## Completion Gate

- Python-versus-Rust parity passes before Python deletion.
- Permanent Rust tests cover every committed golden contract.
- Formatting, Clippy with warnings denied, all workspace tests, and release build pass.
- Identical builds have equal semantic hashes and semantic SQLite rows.
- The scale workload completes under 10 seconds and 512 MiB.
- No NDF Python package, PyYAML lock, database, or cache remains tracked.
- The three-repository plan contains no NDF Python install or import step.
- `v0.1.0` points to the merged Rust commit before PTO-SPEC advances its pin.
