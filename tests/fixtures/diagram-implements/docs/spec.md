---
doc_id: DOC-IMPL-SPEC
status: active
authority: normative
owner: spec
---

# The spec

## The machine halts {#SPEC-HALT-001}
<!-- ndf: kind=requirement modality=must refinement=L1 status=active -->

The machine MUST halt on a trap.

## The halt is absorbing {#SPEC-HALT-002}
<!-- ndf: kind=requirement modality=must refinement=L2 status=active refines=SPEC-HALT-001 -->

Only reset MUST leave the halted state.
