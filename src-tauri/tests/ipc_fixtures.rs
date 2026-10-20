//! Mock fixtures stay in Rust's shapes (`docs/contracts/ipc.md` §7.1).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "integration-test code (tests/): unwrap/expect are denied only outside tests (SR-62)"
)]

mod common;

mod ipc {
    use super::common::{fixtures, table};
    use foreman_app::ipc::COMMANDS;

    /// `ipc::fixture_requests_deserialize`: every request fixture (variants included)
    /// deserializes into its command's request struct, and every command has one.
    #[test]
    fn fixture_requests_deserialize() {
        let files = fixtures("requests");
        let mut failures = Vec::new();
        for (cmd, name, json) in &files {
            if let Err(e) = table::request(cmd, json) {
                failures.push(format!("{name}: {e}"));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
        for c in COMMANDS {
            assert!(
                files.iter().any(|(cmd, _, _)| cmd == c),
                "no request fixture for {c}"
            );
        }
    }

    /// `ipc::fixture_responses_roundtrip`: every response fixture deserializes into its command's
    /// response type and re-serializes to the same JSON value (formatting aside: fixtures are
    /// Prettier-formatted), and every command has one.
    #[test]
    fn fixture_responses_roundtrip() {
        let files = fixtures("responses");
        let mut failures = Vec::new();
        for (cmd, name, json) in &files {
            if let Err(e) = table::response(cmd, json) {
                failures.push(format!("{name}: {e}"));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n\n"));
        for c in COMMANDS {
            assert!(
                files.iter().any(|(cmd, _, _)| cmd == c),
                "no response fixture for {c}"
            );
        }
    }
}
