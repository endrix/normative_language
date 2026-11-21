---
doc_id: DOC-LADDER
status: active
authority: normative
owner: spec
---

# The ladder

## The machine stops {#LAD-000}
<!-- ndf: kind=architecture modality=must refinement=L0 status=active -->

The machine MUST stop when told to.

## A trap is a halt {#LAD-001}
<!-- ndf: kind=requirement modality=must refinement=L1 status=active refines=LAD-000,NEI-002 -->

Entering the trap state MUST stop the machine permanently (see the
schedule): the state is absorbing, and **only** reset leaves it.

## The halt is a register {#LAD-002}
<!-- ndf: kind=requirement modality=must refinement=L2 status=active refines=LAD-001 depends-on=NEI-001 -->

A one-bit register `halted` MUST hold the halt; see [the state](other.md).

## The halt is tested {#LAD-003}
<!-- ndf: kind=verification modality=must refinement=L3 status=active verifies=LAD-001 -->

A directed test MUST trap the machine and observe it stay halted.

## Why it halts {#Q-001}
<!-- ndf: kind=question status=open blocks=LAD-002 -->

Nobody has said whether a halt should raise an interrupt.
