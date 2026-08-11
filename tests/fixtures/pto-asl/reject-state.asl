// PTO-UNIT: {"id":"PTO-ARCH-STATE-OWNER","surface":"arch","classification":["state","owner"],"depends_on":[]}
// PTO-STATE: {"id":"PTO-STATE-ARCH-BAD-SCOPE","classification":["arch","bad"],"scope":"thread","owner":"PTO-ARCH-STATE-OWNER","members":["_Known"],"depends_on":[]}
// PTO-STATE: {"id":"PTO-STATE-ARCH-BAD-OWNER","classification":["arch","owner"],"scope":"core","owner":"PTO-ARCH-OTHER","members":["_Known"],"depends_on":[]}
// PTO-STATE: {"id":"PTO-STATE-ARCH-BAD-MEMBER","classification":["arch","member"],"scope":"core","owner":"PTO-ARCH-STATE-OWNER","members":["_Missing"],"depends_on":[]}
// PTO-STATE: {"id":"PTO-STATE-ARCH-MISSING-FIELD","classification":["arch","schema"],"scope":"core","owner":"PTO-ARCH-STATE-OWNER","members":["_Known"]}
// PTO-STATE: {"id":"PTO-STATE-ARCH-UNKNOWN-FIELD","classification":["arch","schema"],"scope":"core","owner":"PTO-ARCH-STATE-OWNER","members":["_Known"],"depends_on":[],"description":"not allowed"}
// PTO-STATE: {"id":"PTO-STATE-ARCH-DUPLICATE-MEMBER","classification":["arch","member"],"scope":"core","owner":"PTO-ARCH-STATE-OWNER","members":["_Known","_Known"],"depends_on":[]}
// PTO-STATE: {"id":"PTO-STATE-ARCH-DUPLICATE-ID","classification":["arch","identity"],"scope":"core","owner":"PTO-ARCH-STATE-OWNER","members":["_Other"],"depends_on":[]}
// PTO-STATE: {"id":"PTO-STATE-ARCH-DUPLICATE-ID","classification":["arch","identity"],"scope":"core","owner":"PTO-ARCH-STATE-OWNER","members":["_Third"],"depends_on":[]}
// PTO-STATE: {"id":"PTO-STATE-ARCH-ONE","classification":["arch","member"],"scope":"core","owner":"PTO-ARCH-STATE-OWNER","members":["_Known"],"depends_on":[]}
// PTO-STATE: {"id":"PTO-STATE-ARCH-TWO","classification":["arch","member"],"scope":"core","owner":"PTO-ARCH-STATE-OWNER","members":["_Known"],"depends_on":[]}
var _Known : bits(32);
var _Other : bits(32);
var _Third : bits(32);
