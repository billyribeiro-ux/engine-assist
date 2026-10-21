//! ADR-015 layer-1 tooling is wired (M0-05). These are wiring checks only; M1 adds the real
//! goldens (insta) and property tests (proptest) for the template, rules and resolver.

mod tooling {
    use proptest::prelude::*;

    /// insta snapshots work in this crate (inline snapshot; M1 goldens use files under tests/snapshots).
    #[test]
    fn insta_inline_snapshot() {
        insta::assert_snapshot!(foreman_core::version(), @"0.1.0");
    }

    proptest! {
        /// proptest runs in this crate: the version string is stable under any input.
        #[test]
        fn proptest_runs(_seed in any::<u64>()) {
            prop_assert_eq!(foreman_core::version(), env!("CARGO_PKG_VERSION"));
        }
    }
}
