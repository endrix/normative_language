---
doc_id: DOC-IMPL-MODEL
status: active
authority: informative
owner: spec
---

# The model, and what each action implements

## `src/model.py::Model.halt` {#MODEL-HALT}
<!-- ndf: kind=source status=active implements=SPEC-HALT-001,SPEC-HALT-002 -->

Both halt clauses.

## `src/model.py::Model.stay` {#MODEL-STAY}
<!-- ndf: kind=source status=active implements=SPEC-HALT-002 -->

The absorbing state.

## `src/model.py::Model.reset` {#MODEL-RESET}
<!-- ndf: kind=source status=active implements=SPEC-HALT-002 depends-on=SPEC-HALT-001 -->

Leaves the halted state; it also depends on the halt, so it is drawn.
