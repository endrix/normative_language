# PTO ASL Adapter

The PTO adapter normalizes authored PTO-SPEC ASL metadata into canonical NDF
IR. It indexes identity, provenance, relationships, and release-impact facts;
it does not execute, replace, or duplicate architecture semantics. PTO-SPEC
ASL remains the sole authority for instruction behavior and architectural
state transitions.

## Identity model

| PTO entity | Authored form | Canonical kind | Stable attribute |
| --- | --- | --- | --- |
| ASL unit | `PTO-UNIT` or unit fields in `PTO-INSTRUCTION` | `source` | `pto_entity=unit` |
| Synthetic decoder projection | exact dependency `generated:decoders` | `source` | `pto_entity=synthetic-unit` |
| Architecture state family | `PTO-STATE` | `architecture` | `pto_entity=state` |
| Instruction | `PTO-INSTRUCTION` | `definition` | `pto_entity=instruction` |
| Normative contract | `NDF-BEGIN` region | mapped from `kind` | `pto_kind=<kind>` |

An instruction file produces both its ASL unit identity and its derived
`PTO-INST-*` instruction identity. A `defined-in` edge links the instruction
to the unit. Consumers conform to instruction, state, or contract identities;
they do not conform to source-unit nodes.

## ASL unit metadata

Concept units use the existing record:

```asl
// PTO-UNIT: {"id":"PTO-ARCH-PROGRAMMING-MODEL-EXECUTION-CONTEXT","surface":"arch","classification":["programming-model","execution-context"],"depends_on":["PTO-ARCH-SYSTEM-REGISTERS-ADDRESSING"]}
```

Instruction records use the same `id`, `surface`, `classification`, and
`depends_on` fields plus `mnemonic`. Classifications are non-empty lowercase
segments. Every dependency must be another `PTO-*` unit except the exact
virtual name `generated:decoders`, which maps to
`PTO-SYNTHETIC-GENERATED-DECODERS`. Unit dependencies produce `depends-on`
edges and are resolved after the complete project has been parsed.

## Architecture state metadata

State is declared beside its owning ASL variables:

```asl
// PTO-STATE: {"id":"PTO-STATE-ARCH-GPR","classification":["scalar","gpr"],"scope":"pe","owner":"PTO-ARCH-PROGRAMMING-MODEL-EXECUTION-CONTEXT","members":["_PEGPRs"],"depends_on":[]}
var _PEGPRs : array [[PTO_PE_COUNT]] of bits(64);
```

The JSON object has exactly these fields:

- `id`: an uppercase `PTO-STATE-*` identity;
- `classification`: a non-empty array of lowercase segments;
- `scope`: `pe`, `bundle`, `core`, `memory-agent`, `acr`, or `system`;
- `owner`: the same-file unit ID;
- `members`: distinct same-file ASL `var` identifiers, assigned to one state
  family in that owner; and
- `depends_on`: other `PTO-STATE-*` identities.

Each state produces `defined-in` from state to unit, reciprocal `owns-state`
from unit to state, and state `depends-on` edges. Metadata contains no
normative prose; the referenced ASL declarations and accepted transitions
remain authoritative.

## Contracts and instruction facets

The adapter enforces PTO's kind/level pairing, registered layers,
`open|accepted` lifecycle, comment-only NDF region bodies, and `[[PTO-*]]`
references. Instruction identity is derived only from `surface` and
`mnemonic`, so moving a source does not change its
`ndf://pto-spec/PTO-INST-*` URI. Indexed instruction facets use this sorted
vocabulary: `architectural-state`, `completion`, `encoding`, `fault`,
`instruction-semantics`, `legality`, and `memory-ordering`. The adapter does
not infer a facet unsupported by metadata.

## Diagnostics

| Code | Failure |
| --- | --- |
| `NDF-ASL-001`–`NDF-ASL-004` | Existing NDF region and instruction metadata errors |
| `NDF-ASL-005` | Colliding derived instruction identity |
| `NDF-ASL-006` | Invalid or duplicate ASL unit metadata |
| `NDF-ASL-007` | Unresolved or cyclic ASL unit dependency |
| `NDF-ASL-008` | Invalid or duplicate state metadata |
| `NDF-ASL-009` | Invalid state owner, member, dependency, or cycle |

Project validation is deterministic and emits at most one dependency/cycle
diagnostic per invalid source node. File traversal order cannot change the
canonical graph.

## Query and release impact

After `ndf build`, state families are directly queryable:

```bash
ndf query --index graph.sqlite \
  --expression "kind=architecture and attributes.pto_entity=state" \
  --format json
```

Attribute filters use `attributes.<key>=<value>` and unknown top-level fields
fail closed. PTO release impact treats state `scope`, `members`,
`classification`, kind, status, or body changes as breaking for conforming
consumers. A source or unit-owner-only move requires review. Adding only
`PTO-STATE-*` references to `PTO-ARCH-STATE-CLOSURE-001` requires review;
removing a closure reference is breaking.
