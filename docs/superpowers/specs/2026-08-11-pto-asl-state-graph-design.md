# PTO ASL State Graph Design

**Status:** Proposed for written review

**Date:** 2026-08-11

**Repositories:** `PTO-ISA/normative_language`, `PTO-ISA/pto-spec`,
`hengliao1972/DavinciOO`

## 1. Summary

The PTO ASL adapter will index the complete ASL ownership graph and a stable
catalog of architecture-state families in addition to its existing instruction
and NDF-clause nodes. PTO-SPEC ASL remains the only authority for state layout,
legal transitions, faults, completion, and instruction effects. The new
metadata identifies where that authority lives; it does not restate the
semantics.

This replaces the current practice of adding consumer-specific conformance
anchors such as a Shared Tile-only contract. DavinciOO will instead conform to
generic PTO state identities for GPRs, temporary queues, program control,
predicates, faults, memory state, system/trap state, local Tile state, Shared
Tile state, and bundle-control state.

The first rollout rewrites PTO-SPEC PR 66. Its release-event schema remains a
separate, non-normative part of that PR. Its three hand-authored DavinciOO
anchors are replaced by the generic state graph described here.

## 2. Problem

The current adapter produces two public PTO identity classes:

- explicit `// NDF-BEGIN` regions become normative clause nodes; and
- `// PTO-INSTRUCTION` metadata becomes stable `PTO-INST-*` instruction nodes.

Every ASL file also has an existing `PTO-UNIT` identity and dependency list,
but the adapter ignores it. PTO-SPEC therefore has a complete source dependency
graph that cannot be queried through NDF. Architecture state is even less
structured: stable `PTO-REQ-*` comments and ASL declarations exist, but state
families do not have first-class graph identities.

PR 66 exposed this gap by adding a Shared Tile-specific NDF clause while GPR,
T/U queue, local Tile, predicate, PC, memory, and trap state remained implicit.
That design has three defects:

1. it creates upstream identities around one downstream implementation;
2. it confuses an ASL source-unit identity with a conformance contract; and
3. its generated `assert TRUE` compilation point is provenance evidence, not
   semantic conformance evidence.

## 3. Goals

1. Index every PTO ASL unit with its stable existing ID and dependency edges.
2. Give each architecture-state family a stable, queryable PTO identity.
3. Preserve the distinction between source ownership, architecture state,
   normative requirements, and executable instruction semantics.
4. Let downstream NDF clauses use `conforms-to` against generic PTO contracts
   and state identities.
5. Make PTO release impact report state identity removal or semantic metadata
   changes to affected consumers.
6. Keep generated indexes disposable and deterministic.
7. Bring up the feature without changing executable PTO ASL behavior.

## 4. Non-goals

- The adapter does not infer architecture semantics from ASL syntax.
- The adapter does not prove which instructions read or write a state family.
  That requires a later audited call/effect analysis rather than name matching.
- `PTO-UNIT` nodes are not requirements and are not valid substitutes for
  state or instruction conformance targets.
- The first rollout does not assign one node to every ASL variable. State is
  grouped into stable architecture-review families.
- The change does not add a new canonical IR `NodeKind` or change SQLite schema
  version 1.
- The change does not alter PTO instruction encodings, legality, faults,
  results, completion, ordering, or profiles.

## 5. Authority and Identity Classes

The PTO adapter emits four identity classes:

| Entity | Authored form | Canonical kind | Purpose |
| --- | --- | --- | --- |
| ASL unit | `PTO-UNIT` or the `id` inside `PTO-INSTRUCTION` | `source` | Ownership, classification, and ASL dependency graph |
| State family | `PTO-STATE` | `architecture` | Stable identity for programmer-visible state owned by ASL |
| Instruction | derived from `PTO-INSTRUCTION.surface` and `.mnemonic` | `definition` | Stable executable ISA identity and semantic facets |
| Contract | `NDF-BEGIN` region | existing mapped kind | Normative cross-cutting requirement, definition, or mechanism |

The same instruction ASL file produces both a source-unit node, such as
`ndf://pto-spec/PTO-TILE-TLOAD`, and an instruction node, such as
`ndf://pto-spec/PTO-INST-TILE-TLOAD`. The instruction node has a `defined-in`
edge to the source-unit node. Consumers conform to the instruction node, not
the source-unit node.

No new IR kind is necessary. Nodes carry a stable adapter attribute:

```text
pto_entity=unit | synthetic-unit | state | instruction | clause
```

## 6. Authored PTO Metadata

### 6.1 ASL unit metadata

Existing metadata remains authoritative and unchanged:

```asl
// PTO-UNIT: {"id":"PTO-ARCH-PROGRAMMING-MODEL-EXECUTION-CONTEXT","surface":"arch","classification":["programming-model","execution-context"],"depends_on":["PTO-ARCH-SYSTEM-REGISTERS-ADDRESSING"]}
```

For `PTO-INSTRUCTION` files, the existing JSON `id`, `surface`,
`classification`, and `depends_on` fields define the unit node. The adapter
must not require a second `PTO-UNIT` line.

Unit-node attributes are:

```text
pto_entity=unit
surface=<arch|block|scalar|tile>
classification=<slash-separated classification>
mnemonic=<mnemonic when present>
```

Each `depends_on` entry becomes a `depends-on` edge between unit nodes.

PTO-SPEC has one existing virtual dependency name, `generated:decoders`. The
adapter maps that exact spelling to the canonical source node
`ndf://pto-spec/PTO-SYNTHETIC-GENERATED-DECODERS` with
`pto_entity=synthetic-unit`. The node has virtual source path
`generated:decoders`; it denotes the deterministic decoder projection and does
not make generated output normative. Any other non-`PTO-*` dependency is an
error.

### 6.2 State-family metadata

An ASL owner may declare multiple state families:

```asl
// PTO-STATE: {"id":"PTO-STATE-ARCH-GPR","classification":["scalar","gpr"],"scope":"pe","owner":"PTO-ARCH-PROGRAMMING-MODEL-EXECUTION-CONTEXT","members":["_PEGPRs"],"depends_on":[]}
```

Required fields are:

- `id`: stable `PTO-STATE-*` identity;
- `classification`: non-empty array of lowercase classification segments;
- `scope`: one of `pe`, `bundle`, `core`, `memory-agent`, `acr`, or `system`;
- `owner`: the `PTO-UNIT`/instruction unit ID in the same file;
- `members`: non-empty array of distinct ASL identifiers declared in that
  owner file; and
- `depends_on`: array of other `PTO-STATE-*` identities.

The adapter emits:

- one `architecture` node with `pto_entity=state`;
- one `defined-in` edge from the state node to its unit node;
- one `owns-state` edge from the unit node to the state node; and
- one `depends-on` edge for each state dependency.

State-node attributes are deterministic strings:

```text
pto_entity=state
classification=<slash-separated classification>
scope=<scope>
members=<comma-separated members in authored order>
```

The node body is empty. ASL declarations and procedures remain the sole state
semantics; metadata must not contain normative prose.

### 6.3 Initial state catalog

The PTO-SPEC rollout publishes these families:

| Stable ID | Owner | Initial members |
| --- | --- | --- |
| `PTO-STATE-ARCH-GPR` | execution context | `_PEGPRs` |
| `PTO-STATE-ARCH-TEMPORARY-QUEUES` | execution context | `_TQueue`, `_UQueue`; scope is `bundle` because ASL defines them as bundle-local queues |
| `PTO-STATE-ARCH-PROGRAM-CONTROL` | execution context | `_PC`, `_BPC`, bundle-active flags, return/commit values, execution mask, predicates |
| `PTO-STATE-ARCH-FAULT` | execution context | `_LastFault`, `_FaultAddress` |
| `PTO-STATE-ARCH-MEMORY` | execution context | memory, reservation, fence, and memory-event state |
| `PTO-STATE-ARCH-MAINTENANCE` | execution context | cache/TLB epochs, maintenance, architecture/control request state |
| `PTO-STATE-ARCH-SYSTEM-REGISTERS` | execution context | extended system registers and current ACR |
| `PTO-STATE-ARCH-TRAP-CONTEXT` | execution context | ACR trap state and trap contexts |
| `PTO-STATE-TILE-LOCAL` | local Tile registers | `_Tiles`, `_TileAllocationMasks` |
| `PTO-STATE-TILE-SHARED` | local Tile registers | `_SharedTiles` |
| `PTO-STATE-BLOCK-CONTROL` | block control state | the declared `_Bundle*`, frame, queue, memory-command, cross-block, and hint state in that unit |

The implementation must copy exact member identifiers from the owning ASL
declarations. It must not use the abbreviated prose in this table as parser
input.

### 6.4 State closure contract

`PTO-ARCH-STATE-CLOSURE-001` remains a normative NDF contract. Its body
references every initial `PTO-STATE-*` identity and states only the closure
rule: programmer-visible state is the referenced ASL-owned state, and accepted
ASL transitions are the only way to change it.

Existing `PTO-REQ-STATE-001`, `PTO-REQ-TILE-001`,
`PTO-REQ-SHARED-TILE-001`, and `PTO-REQ-BUNDLE-STATE-001` remain requirement
identities and will be migrated from unparsed comments into explicit NDF
regions at their canonical ASL owners. They reference the matching state
nodes. Requirement identities and state identities are deliberately separate.

## 7. Validation and Diagnostics

The adapter fails closed for:

- malformed or non-object metadata;
- missing, additional, or wrongly typed fields;
- duplicate unit or state identities;
- an instruction unit ID that collides with another unit;
- a unit dependency that does not resolve inside the complete PTO project;
- a non-`PTO-*` unit dependency other than the exact supported
  `generated:decoders` alias;
- a state owner that differs from the same-file unit ID;
- a state member that is not declared by an ASL `var` declaration in the owner
  file;
- duplicate state members within one family or across families in one owner;
- unknown state dependencies; and
- dependency cycles in the unit or state graph.

New stable diagnostic codes are:

| Code | Meaning |
| --- | --- |
| `NDF-ASL-006` | invalid or duplicate ASL unit metadata |
| `NDF-ASL-007` | unresolved or cyclic ASL unit dependency |
| `NDF-ASL-008` | invalid or duplicate PTO state metadata |
| `NDF-ASL-009` | invalid state owner, member, dependency, or cycle |

Existing `NDF-ASL-001` through `NDF-ASL-005` retain their meanings.
Diagnostics include the metadata source span and a deterministic repair
message. Repository-wide resolution occurs after every file has been parsed,
so file parse order cannot affect results.

## 8. Compiler and Query Behavior

The Rust PTO adapter first parses file-local metadata into nodes and unresolved
edges. Repository indexing then resolves and validates the complete graph.
Deterministic ordering remains by canonical node URI and
`(source, edge type, target)`.

Representative queries are:

```bash
ndf show ndf://pto-spec/PTO-STATE-TILE-SHARED --root isa --format json
ndf trace ndf://pto-spec/PTO-ARCH-STATE-CLOSURE-001 --edges references --root isa --format json
ndf query --root isa --kind architecture --attribute pto_entity=state --format json
```

No query reads ASL directly after index construction. SQLite remains a derived
cache and is never committed.

## 9. Release Impact Semantics

For a consumer with `conforms-to` edges:

- removing a conformance target is `breaking`;
- changing a state node's `scope` or `members` is `breaking`;
- changing a state ID is removal plus addition and therefore `breaking`;
- moving a state owner while preserving ID, scope, members, and ASL semantics
  is `review-required` because provenance changed;
- adding an unrelated unit or state is `compatible`; and
- adding a new referenced state to `PTO-ARCH-STATE-CLOSURE-001` is
  `review-required` for consumers of the closure contract.

The impact engine reports facts from canonical nodes and edges. It does not
claim that unchanged metadata proves unchanged ASL behavior; exact PTO commit
review and ASL evidence remain required.

## 10. DavinciOO Consumption

DavinciOO retains a broad conformance edge from `ISA-STATE-001` to
`PTO-ARCH-STATE-CLOSURE-001`. More specific microarchitecture clauses map to
state identities:

- scalar register/rename clauses conform to `PTO-STATE-ARCH-GPR`;
- queue handling conforms to `PTO-STATE-ARCH-TEMPORARY-QUEUES`;
- Tile register/rename clauses conform to `PTO-STATE-TILE-LOCAL`;
- StgBufB/SlotID clauses conform to `PTO-STATE-TILE-SHARED`;
- control, ROB, and retirement clauses conform to applicable program-control,
  fault, memory, system-register, trap-context, and block-control state nodes.

DavinciOO does not conform to PTO unit nodes. Unit nodes provide source
provenance when tracing a state or contract target.

## 11. Three-Repository Rollout

### 11.1 `PTO-ISA/normative_language`

1. Add failing PTO adapter fixtures for unit and state metadata.
2. Parse unit/state nodes and edges without changing canonical IR schema.
3. Add repository-wide resolution, cycle, owner, and member validation.
4. Extend export, query, dependency, and PTO release-impact golden tests.
5. Update the PTO adapter documentation and synchronized English/Chinese NDF
   specification where the supported authored forms are described.
6. Merge and record the exact compiler commit.

### 11.2 `PTO-ISA/pto-spec` PR 66

1. Replace the three consumer-specific anchor tests with generic unit/state
   graph contract tests.
2. Add `PTO-STATE` records to canonical state owner files.
3. Migrate the four existing state requirement comments into explicit NDF
   regions and make the closure contract reference the full state catalog.
4. Remove `PTO-ARCH-SHARED-TILE-STATE-001`.
5. Keep executable ASL unchanged and regenerate derived documentation and
   release evidence.
6. Keep the release-event schema commit separate in history and describe it as
   a downstream delivery contract, not an architecture-state change.
7. Rewrite the PR body around the generic state graph and exact NDF compiler
   prerequisite.

### 11.3 `hengliao1972/DavinciOO`

1. Advance `toolchains/ndf` to the merged compiler commit.
2. Advance `isa` to the rewritten PR 66 merge commit only after it merges.
3. Replace Shared Tile-only mappings with the state mappings in Section 10.
4. Rebuild the NDF index, run dependency and impact checks, and publish the
   compatibility report.

Downstream pins advance only to merged 40-character commits. Local worktrees
may test unmerged tuples, but such tuples are marked non-release and are not
committed to `ndf.lock`.

## 12. Test Strategy

### 12.1 NDF compiler

- golden parse test for one concept unit, one instruction unit, multiple state
  records, explicit clauses, and all emitted edges;
- negative tests for each diagnostic in Section 7;
- move test proving explicit unit/state/instruction IDs survive path changes;
- deterministic index test over repeated builds;
- query tests for `pto_entity=state` and state-to-unit provenance;
- dependency graph test loading PTO as an exact external project; and
- impact tests for compatible, review-required, and breaking state changes.

Every production change follows red-green-refactor. A new test must fail for
the intended missing behavior before implementation.

### 12.2 PTO-SPEC

- Python parity tests require the same unit/state identities and edges as the
  pinned Rust adapter;
- layout checks require every `PTO-STATE` owner and member to resolve;
- NDF checks require every closure reference to resolve;
- generated documentation checks prove ASL mirrors remain current;
- existing ASL executable tests remain unchanged and passing; and
- release evidence records state nodes as architecture metadata, not as
  automatically satisfied executable requirements.

### 12.3 DavinciOO

- project-contract tests require exact compiler and PTO pins;
- conformance tests require every architecture-state implementation clause to
  target the intended PTO state identity;
- the complete NDF graph must have zero new diagnostics; and
- the PTO release-impact report must classify the accepted tuple without
  rewriting DavinciOO normative bodies.

## 13. Alternatives Considered

### 13.1 Treat every `PTO-UNIT` as a requirement

Rejected. It would convert source ownership into normative conformance and
make compilation-only evidence appear to satisfy behavior.

### 13.2 Keep only one state-closure clause

Rejected. It cannot answer which state family a microarchitecture structure
implements and recreates the Shared Tile inconsistency at a larger scope.

### 13.3 Add a new canonical IR `State` kind

Deferred. `NodeKind::Architecture` plus `pto_entity=state` expresses the needed
queries without a schema migration. A future generic state ontology may add a
new IR kind only after at least one non-PTO adapter demonstrates the same need.

### 13.4 Infer state families and effects from ASL syntax

Deferred. Textual inference would be fragile and could silently manufacture
architecture claims. Explicit state-family metadata is reviewable; audited
effect analysis can be added later as derived evidence.

## 14. Completion Criteria

The bring-up is complete when:

1. the merged Rust compiler indexes unit, state, instruction, and clause nodes;
2. PTO-SPEC PR 66 publishes the complete initial state catalog without changing
   executable ASL semantics;
3. PR 66 no longer contains a DavinciOO-specific Shared Tile anchor;
4. DavinciOO resolves and uses the new state identities through exact pins;
5. all three repositories pass their documented local gates; and
6. the compatibility report distinguishes source provenance, state identity,
   requirement conformance, and executable instruction semantics.
