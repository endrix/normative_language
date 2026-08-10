# PTO ASL Adapter

The PTO adapter normalizes `// NDF-BEGIN` regions and
`// PTO-INSTRUCTION` metadata into the canonical NDF graph. It does not change
or duplicate PTO architecture semantics: PTO-SPEC ASL remains authoritative.

The adapter enforces PTO's current kind/level pairing, registered layers,
`open|accepted` lifecycle, comment-only region bodies, stable instruction
identities, and `[[PTO-*]]` references. It returns canonical nodes, edges, and
stable diagnostics; PTO-specific parser dataclasses are not part of the public
NDF model.
