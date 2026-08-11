# PTO ASL Adapter

The PTO adapter normalizes `// NDF-BEGIN` regions and
`// PTO-INSTRUCTION` metadata into the canonical NDF graph. It does not change
or duplicate PTO architecture semantics: PTO-SPEC ASL remains authoritative.

The adapter enforces PTO's current kind/level pairing, registered layers,
`open|accepted` lifecycle, comment-only region bodies, stable instruction
identities, and `[[PTO-*]]` references. It returns canonical nodes, edges, and
stable diagnostics; PTO-specific parser dataclasses are not part of the public
NDF model.

Instruction identity is derived only from `surface` and `mnemonic`, so moving
an ASL source does not change its `ndf://pto-spec/PTO-INST-*` URI. The adapter
indexes the instruction's semantic handler, catalog status, and every metadata-
supported facet from this sorted vocabulary: `architectural-state`,
`completion`, `encoding`, `fault`, `instruction-semantics`, `legality`, and
`memory-ordering`. It does not infer a facet that the metadata does not support.
Two declarations whose different surface/mnemonic spellings collapse to the
same stable URI are rejected with `NDF-ASL-005`.
