// Wires the cross-cutting F3 suite (tests/security/traversal.rs) into Cargo.
// The workspace root has no package, so a crate integration target is required.
include!("../../../tests/security/traversal.rs");
