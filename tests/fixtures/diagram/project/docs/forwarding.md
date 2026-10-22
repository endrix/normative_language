---
doc_id: DOC-FORWARDING
status: active
authority: normative
owner: forwarding
---

# Forwarding

## Forwarding intent {#FWD-ADM-000}
<!-- ndf: kind=architecture refinement=L0 domain=forwarding status=active -->

The switch admits a frame to the forwarding pipeline only when it is well
formed, and drops it otherwise.

## Frame admission {#FWD-ADM-001}
<!-- ndf: kind=requirement modality=must refinement=L1 domain=forwarding status=active refines=FWD-ADM-000 depends-on=PRM-BUF-001 -->

A frame MUST be admitted only if it passes FCS validation ([[MISSING-001]])
and fits the admission buffer.

## Admission under pressure {#FWD-ADM-002}
<!-- ndf: kind=requirement modality=should refinement=L1 domain=forwarding status=draft conflicts-with=FWD-ADM-001 depends-on=MISSING-001 -->

Under back pressure the switch SHOULD admit every frame and defer the check.

## Which admission policy wins {#Q-001}
<!-- ndf: kind=question status=open domain=forwarding blocks=FWD-ADM-002 -->

Admission under pressure contradicts frame admission; one of them has to go.

## Admission conformance {#VER-ADM-001}
<!-- ndf: kind=verification modality=must refinement=L3 domain=forwarding status=active verifies=FWD-ADM-001 -->

A directed test MUST present one malformed frame per thousand and observe
every one of them dropped.
