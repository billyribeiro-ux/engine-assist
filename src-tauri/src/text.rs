//! Foreman-written text the views show (SR-56: the webview never composes text from repo content).
//! Stage subtitles, condition labels and effects, and project-variable labels, as in the mockups
//! (`docs/design/mockups/`).

use foreman_core::ids::ConditionId;

/// The one-line purpose under each stage title (Stages 0–18).
pub const STAGE_SUBTITLES: [&str; 19] = [
    "Find where the money is.",
    "Interview, brainstorm, and write the App Brief in one claude.ai chat.",
    "Design the entire product. Council in Claude Code, then freeze the Master Spec.",
    "Business, security, privacy, legal.",
    "The complete build package.",
    "Repo, CI, environments.",
    "Database, auth, domain core.",
    "The walking skeleton.",
    "Security controls from the threat model.",
    "Data flows and integrations.",
    "Payments and entitlements.",
    "Observability, SLOs, and error budgets.",
    "Complete every feature.",
    "Design system, polish, accessibility.",
    "Independent audits.",
    "Copy, onboarding, growth surfaces.",
    "Readiness review, drills, dogfood, beta.",
    "Ring-based launch and revenue.",
    "Operate and grow.",
];

/// The subtitle of a stage.
#[must_use]
pub fn stage_subtitle(n: u8) -> &'static str {
    STAGE_SUBTITLES.get(usize::from(n)).copied().unwrap_or("")
}

/// A condition's question and what "yes" adds (DR-027).
#[must_use]
pub const fn condition(c: ConditionId) -> (&'static str, &'static str) {
    match c {
        ConditionId::IdeaKnown => (
            "You already know the idea",
            "Yes skips Stage 0, Land Survey.",
        ),
        ConditionId::Commercial => (
            "You'll sell it",
            "Adds the demand test, competitor teardown, pricing and marketing pages.",
        ),
        ConditionId::Payments => (
            "It takes payments",
            "Adds Stage 10, Electrical, and live payments at launch.",
        ),
        ConditionId::MultiUser => (
            "More than one person signs in",
            "Adds authentication, onboarding, the beta ring and analytics.",
        ),
        ConditionId::Public => (
            "Anyone on the internet can reach it",
            "Adds authentication hardening and public-facing reviews.",
        ),
        ConditionId::HasUi => (
            "It has a user interface",
            "Adds 4.5 design system and Stage 13 accessibility.",
        ),
        ConditionId::OthersData => (
            "It stores other people's data",
            "Adds the legal part of 3.2 and legal pages.",
        ),
        ConditionId::Server => (
            "It runs on a server",
            "Adds the staging pipeline, SLOs and load tests.",
        ),
        ConditionId::HasDatabase => ("It has a database", "Adds 6.1 migrations."),
        ConditionId::HasUploads => ("It takes uploads", "Adds the uploads part of Stage 9."),
        ConditionId::HasEmail => ("It sends email", "Adds the email part of Stage 9."),
        ConditionId::HasIntegrations => (
            "It integrates other services",
            "Adds integration tests to Stage 9.",
        ),
    }
}

/// A project variable the setup form asks for: (name, label, placeholder, multiline, from
/// Settings). `APP_NAME` is the project name, so it isn't listed.
pub const VARIABLES: [(&str, &str, &str, bool, bool); 6] = [
    (
        "ROUGH_IDEA",
        "Rough idea",
        "<one or two sentences>",
        true,
        false,
    ),
    (
        "TARGET_USERS",
        "Who it's for",
        "<who it's for>",
        false,
        false,
    ),
    (
        "MY_ADVANTAGES",
        "Your advantages",
        "<what you bring>",
        true,
        false,
    ),
    (
        "BUDGET_TIME",
        "Budget and time",
        "<hours per week, money>",
        false,
        false,
    ),
    (
        "PREFERRED_STACK",
        "Preferred stack",
        "<your stack>",
        false,
        true,
    ),
    ("QUALITY_BAR", "Quality bar", "<quality bar>", false, true),
];
