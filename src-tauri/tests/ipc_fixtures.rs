//! Mock fixtures stay in Rust's shapes (`docs/contracts/ipc.md` §7.1).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "integration-test code (tests/): unwrap/expect are denied only outside tests (SR-62)"
)]

mod ipc {
    use foreman_app::ipc::commands::app::{AppBootstrap, AppBootstrapReq};
    use serde_json::Value;

    const REQ_APP_BOOTSTRAP: &str =
        include_str!("../../src/lib/ipc/fixtures/requests/app_bootstrap.json");
    const RES_APP_BOOTSTRAP: &str =
        include_str!("../../src/lib/ipc/fixtures/responses/app_bootstrap.json");

    /// `ipc::fixture_requests_deserialize`: every request fixture deserializes into its struct.
    #[test]
    fn fixture_requests_deserialize() {
        let _: AppBootstrapReq = serde_json::from_str(REQ_APP_BOOTSTRAP).unwrap();
    }

    /// `ipc::fixture_responses_roundtrip`: every response fixture deserializes and re-serializes
    /// to the same JSON value (formatting aside: fixtures are Prettier-formatted).
    #[test]
    fn fixture_responses_roundtrip() {
        let typed: AppBootstrap = serde_json::from_str(RES_APP_BOOTSTRAP).unwrap();
        let original: Value = serde_json::from_str(RES_APP_BOOTSTRAP).unwrap();
        assert_eq!(serde_json::to_value(&typed).unwrap(), original);
    }
}
