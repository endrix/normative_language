---
doc_id: DOC-FIXTURE-PIPELINE
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

## PTO conformance {#PIPE-PTO-001}
<!-- ndf: kind=requirement modality=must refinement=L1 domain=core status=active conforms-to=ndf://pto-spec/PTO-INST-TILE-TLOAD -->

The pipeline MUST preserve the PTO load contract.

## Prospective PTO extension {#PIPE-PTO-EXT-001}
<!-- ndf: kind=requirement modality=tbd refinement=L1 domain=core status=draft tracks=ndf://pto-spec/PTO-INST-TILE-PEXT expected=absent -->

The extension remains outside the accepted PTO release.
