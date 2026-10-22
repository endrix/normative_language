---
doc_id: DOC-OPTIONS
status: active
authority: normative
owner: forwarding
---

# Design options

## Admission buffer depth {#PRM-BUF-001}
<!-- ndf: kind=option modality=tbd refinement=L2 domain=forwarding status=draft couples-with=PRM-THR-001 -->

The admission buffer depth is a design-space parameter: default 64 frames,
explored over 32 to 128.

## Concurrent threads {#PRM-THR-001}
<!-- ndf: kind=option modality=tbd refinement=L2 domain=forwarding status=draft -->

The number of concurrent forwarding threads trades off against the buffer
depth under a fixed memory budget.
