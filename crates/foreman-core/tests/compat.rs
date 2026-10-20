//! M1-20: the Claude Code version floor (ADR-017, AC-F008-02).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code"
)]
#![allow(
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use foreman_core::compat as fc;
use foreman_core::ids::{Effort, Model};
use foreman_core::schema::project::{ModelPolicy, Plan, Substitution};
use semver::Version;

mod compat {
    use super::*;

    #[test]
    fn parses_version_output() {
        assert_eq!(
            fc::parse_version_output("2.1.211 (Claude Code)"),
            Some(Version::new(2, 1, 211))
        );
        assert_eq!(
            fc::parse_version_output("2.1.304 (Claude Code)\n"),
            Some(Version::new(2, 1, 304))
        );
        assert_eq!(
            fc::parse_version_output("v2.1.293"),
            Some(Version::new(2, 1, 293))
        );
        assert_eq!(fc::parse_version_output("claude: command not found"), None);
        assert_eq!(fc::parse_version_output(""), None);
        // AC-F008-02: the floor is the maximum (2.1.293 with the current kit's Haiku agents).
        let t = common::parse(&common::blueprint());
        let max = ModelPolicy {
            plan: Plan::Max,
            substitutions: vec![],
        };
        let kit = [Model::Haiku, Model::Sonnet, Model::Opus];
        let (floor, reqs) = fc::floor(&t, &kit, &max);
        assert_eq!(floor, Version::new(2, 1, 293));
        assert!(
            reqs.iter()
                .any(|r| r.feature == "Fable 5.1" && r.min == "2.1.257")
        );
        // Without the kit, Fable/Opus/Sonnet steps alone need 2.1.284.
        let (f2, _) = fc::floor(&t, &[], &max);
        assert_eq!(f2, Version::new(2, 1, 284));
        // A substitution that removes Fable removes its requirement.
        let sub = ModelPolicy {
            plan: Plan::Max,
            substitutions: vec![Substitution {
                from: Model::Fable,
                to: Model::Opus,
                effort: Some(Effort::Xhigh),
            }],
        };
        assert!(
            !fc::floor(&t, &[], &sub)
                .1
                .iter()
                .any(|r| r.feature == "Fable 5.1")
        );
        assert!(fc::meets(&Version::new(2, 1, 304), &floor));
        assert!(!fc::meets(&Version::new(2, 1, 292), &floor));
    }
}
