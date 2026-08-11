// PTO-INSTRUCTION: {"id":"PTO-TILE-TLOAD","surface":"tile","classification":["memory-and-data-movement","regular"],"depends_on":["PTO-TILE-MODEL-MEMORY-LOAD-STORE","generated:decoders"],"mnemonic":"TLOAD","catalog_records":[{"semantic_handler":"TLOAD","contract_status":"reviewed-complete","state_effects":["operand:destination0:destination"]}]}

// NDF-BEGIN: PTO-TILE-CAPACITY
// ndf: kind=contract level=L1 layer=tile status=accepted
// Tile capacity is defined per selected PE; see [[PTO-INST-TILE-TLOAD]].
// NDF-END: PTO-TILE-CAPACITY

// NDF-BEGIN: PTO-TILE-CAPACITY-MECHANISM
// ndf: kind=mechanism level=L2 layer=tile status=open
// The implementation tracks capacity in tile state.
// NDF-END: PTO-TILE-CAPACITY-MECHANISM
