// PTO-INSTRUCTION: {"assembly":["TLOAD <bundle operands>"],"catalog_records":[{"contract_status":"reviewed-complete","effect_contract":"TLOAD","fault_contract":"ExecuteTileInstruction","function":0,"legality_handler":"TileOperandsLegal_TLOAD","memory_ordering_contract":"PTO-TSO","restart_contract":"CompleteBundleAtWithAcceptedApplicabilityRules","selector":"0x078","semantic_handler":"TLOAD","state_effects":["operand:destination0:destination"]}],"mnemonic":"TLOAD","surface":"tile"}

// NDF-BEGIN: PTO-TILE-CAPACITY
// ndf: kind=contract level=L1 layer=tile status=accepted
// Tile capacity is defined per selected PE; see [[PTO-INST-TILE-TLOAD]].
// NDF-END: PTO-TILE-CAPACITY

// NDF-BEGIN: PTO-TILE-CAPACITY-MECHANISM
// ndf: kind=mechanism level=L2 layer=tile status=open
// The implementation tracks capacity in tile state.
// NDF-END: PTO-TILE-CAPACITY-MECHANISM
