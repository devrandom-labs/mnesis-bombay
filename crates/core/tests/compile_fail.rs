//! Compile-fail proofs for illegal protocol compositions.

#[test]
fn typed_protocol_rejects_invalid_compositions() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/*.rs");
}
