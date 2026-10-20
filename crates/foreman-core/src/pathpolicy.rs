//! The lexical path policy (ADR-011 layer 1, SR-30, SR-31, SR-50; DR-160, DR-161). Pure functions:
//! no filesystem access. `store` resolves the accepted paths component by component.
//!
//! Every component is compared by its key `fold_string(nfc(c))` (ICU4X full case folding after NFC),
//! so `.GIT`, `Claude.MD`, a Kelvin-sign `.husKy` and NFD forms all match the denied names.

use std::fmt;

use icu_casemap::CaseMapper;
use icu_normalizer::ComposingNormalizer;
use serde::{Deserialize, Serialize};

/// Longest accepted path, in bytes (SR-30).
pub const MAX_PATH_BYTES: usize = 1024;
/// Longest accepted component, in bytes (SR-30).
pub const MAX_COMPONENT_BYTES: usize = 255;

/// Components denied at any depth (SR-30), as folded keys.
pub const DENIED_COMPONENTS: [&str; 7] = [
    ".git", ".claude", ".husky", ".vscode", ".idea", ".github", ".foreman",
];
/// File names denied at any depth (SR-30), as folded keys. `CLAUDE.md` is handled separately: it is
/// allowed only at the root, through the assembler.
pub const DENIED_NAMES: [&str; 6] = [
    ".mcp.json",
    "claude.local.md",
    ".envrc",
    ".gitmodules",
    ".gitattributes",
    ".npmrc",
];

/// The six council agents (DR-036): the only `spec/.claude/agents/<name>.md` destinations.
pub const COUNCIL_AGENTS: [&str; 6] = [
    "market-analyst",
    "user-advocate",
    "product-visionary",
    "principal-engineer",
    "growth-strategist",
    "red-team",
];

/// The comparison key of a component: NFC, then full case folding.
#[must_use]
pub fn key(component: &str) -> String {
    let nfc = ComposingNormalizer::new_nfc().normalize(component);
    CaseMapper::new().fold_string(&nfc).into_owned()
}

/// A validated, repo-relative (or app-data-relative) path: `/`-separated, no empty, `.` or `..`
/// components.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RepoPath(String);

impl RepoPath {
    /// The path as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The components.
    pub fn components(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }

    /// The parent components and the final name.
    #[must_use]
    pub fn split_last(&self) -> (Vec<&str>, &str) {
        let mut c: Vec<&str> = self.0.split('/').collect();
        let last = c.pop().unwrap_or_default();
        (c, last)
    }

    /// Builds a path from trusted Foreman constants (app-data layout). Still syntax-checked.
    ///
    /// # Errors
    /// When the text fails the syntax rules.
    pub fn app_data(rel: &str) -> Result<Self, PolicyViolation> {
        check_app_data(rel)
    }
}

impl fmt::Display for RepoPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The kind of a decision file written under `.foreman/` (ADR-011 typed exceptions).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecisionPath {
    /// `.foreman/project.json`
    Project,
    /// `.foreman/overlay.json`
    Overlay,
    /// `.foreman/blueprint.lock.json`
    Lock,
    /// `.foreman/stages/stage-NN.json`
    StageRecord(u8),
}

impl DecisionPath {
    /// The exact repo path.
    #[must_use]
    pub fn path(self) -> String {
        match self {
            Self::Project => ".foreman/project.json".into(),
            Self::Overlay => ".foreman/overlay.json".into(),
            Self::Lock => ".foreman/blueprint.lock.json".into(),
            Self::StageRecord(n) => format!(".foreman/stages/stage-{n:02}.json"),
        }
    }
}

/// Why a write happens. Exceptions to the deny rules are typed variants, never strings from
/// content (ADR-011).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WritePurpose {
    /// An ordinary repo write: under `docs/` or `spec/` only.
    Repo,
    /// An annotation `out`/`appends` path (DR-026): the same rules, no exceptions.
    TemplateOut,
    /// `council-setup`: `spec/.claude/agents/<agent>.md` for one of [`COUNCIL_AGENTS`] (index).
    CouncilSetup(usize),
    /// A decision-file writer.
    DecisionFile(DecisionPath),
    /// Root `CLAUDE.md`, after the native confirmation.
    AssemblerClaudeMd,
}

/// What a read is for; decides the size cap (SR-31).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReadClass {
    /// Packets, attachments, lessons, decision files, approvals: 1 MB.
    Decision,
    /// Claude configuration: 256 KB (larger blocks the launch).
    ClaudeConfig,
    /// The blueprint and app-data templates: 8 MB.
    Template,
    /// Other app-data state (progress, run records): 8 MB.
    AppState,
}

impl ReadClass {
    /// The size cap in bytes.
    #[must_use]
    pub const fn cap(self) -> u64 {
        match self {
            Self::Decision => 1024 * 1024,
            Self::ClaudeConfig => 256 * 1024,
            Self::Template | Self::AppState => 8 * 1024 * 1024,
        }
    }
}

/// A refused path, with the rule that refused it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PolicyViolation {
    /// Empty path.
    #[error("empty path")]
    Empty,
    /// Over 1,024 bytes.
    #[error("path is {0} bytes (limit 1,024)")]
    TooLong(usize),
    /// A component over 255 bytes.
    #[error("a path component is {0} bytes (limit 255)")]
    ComponentTooLong(usize),
    /// Starts with `/` or `~`.
    #[error("absolute paths are not allowed")]
    Absolute,
    /// A `..` component.
    #[error("`..` is not allowed")]
    DotDot,
    /// An empty or `.` component.
    #[error("empty or `.` path component")]
    EmptyComponent,
    /// A NUL byte.
    #[error("NUL character in path")]
    Nul,
    /// C0, DEL or C1.
    #[error("control character U+{0:04X} in path")]
    Control(u32),
    /// A bidi control.
    #[error("bidirectional control U+{0:04X} in path")]
    Bidi(u32),
    /// A zero-width character.
    #[error("zero-width character U+{0:04X} in path")]
    ZeroWidth(u32),
    /// A backslash.
    #[error("backslash in path")]
    Backslash,
    /// A denied component (`.git`, `.claude`, …) at any depth.
    #[error("`{0}` is a protected folder")]
    DeniedComponent(String),
    /// A denied file name (`.mcp.json`, …).
    #[error("`{0}` is a protected file")]
    DeniedName(String),
    /// `CLAUDE.md` outside the assembler's root write.
    #[error("CLAUDE.md is written only at the root, by the assembler, after confirmation")]
    ClaudeMd,
    /// Not under `docs/` or `spec/` and no typed exception.
    #[error("Foreman writes only under docs/ and spec/")]
    NotAllowlisted,
    /// A secret-file pattern (SR-50).
    #[error("`{0}` matches a secret-file pattern")]
    Secret(String),
    /// A symlink at a component (store walk).
    #[error("`{0}` is a symlink")]
    Symlink(String),
}

impl PolicyViolation {
    /// The stable rule code.
    #[must_use]
    pub fn rule(&self) -> &'static str {
        match self {
            Self::Empty => "path.empty",
            Self::TooLong(_) => "path.too-long",
            Self::ComponentTooLong(_) => "path.component-too-long",
            Self::Absolute => "path.absolute",
            Self::DotDot => "path.dotdot",
            Self::EmptyComponent => "path.empty-component",
            Self::Nul => "path.nul",
            Self::Control(_) => "path.control",
            Self::Bidi(_) => "path.bidi",
            Self::ZeroWidth(_) => "path.zero-width",
            Self::Backslash => "path.backslash",
            Self::DeniedComponent(_) => "path.denied-component",
            Self::DeniedName(_) => "path.denied-name",
            Self::ClaudeMd => "path.claude-md",
            Self::NotAllowlisted => "path.not-allowlisted",
            Self::Secret(_) => "path.secret",
            Self::Symlink(_) => "path.symlink",
        }
    }
}

/// True for bidi controls (U+202A–U+202E, U+2066–U+2069, U+200E/F, U+061C).
#[must_use]
pub fn is_bidi(c: char) -> bool {
    matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}' | '\u{061C}')
}

/// True for zero-width characters (U+200B–U+200D, U+2060, U+FEFF).
#[must_use]
pub fn is_zero_width(c: char) -> bool {
    matches!(c, '\u{200B}'..='\u{200D}' | '\u{2060}' | '\u{FEFF}')
}

/// Syntax rules shared by every check; returns the components.
fn syntax(rel: &str) -> Result<Vec<&str>, PolicyViolation> {
    if rel.is_empty() {
        return Err(PolicyViolation::Empty);
    }
    if rel.len() > MAX_PATH_BYTES {
        return Err(PolicyViolation::TooLong(rel.len()));
    }
    for c in rel.chars() {
        let cp = u32::from(c);
        if c == '\0' {
            return Err(PolicyViolation::Nul);
        }
        if cp < 0x20 || cp == 0x7f || (0x80..=0x9f).contains(&cp) {
            return Err(PolicyViolation::Control(cp));
        }
        if is_bidi(c) {
            return Err(PolicyViolation::Bidi(cp));
        }
        if is_zero_width(c) {
            return Err(PolicyViolation::ZeroWidth(cp));
        }
        if c == '\\' {
            return Err(PolicyViolation::Backslash);
        }
    }
    if rel.starts_with('/') || rel.starts_with('~') {
        return Err(PolicyViolation::Absolute);
    }
    let comps: Vec<&str> = rel.split('/').collect();
    for c in &comps {
        if *c == ".." {
            return Err(PolicyViolation::DotDot);
        }
        if c.is_empty() || *c == "." {
            return Err(PolicyViolation::EmptyComponent);
        }
        if c.len() > MAX_COMPONENT_BYTES {
            return Err(PolicyViolation::ComponentTooLong(c.len()));
        }
    }
    Ok(comps)
}

/// Checks one component against the deny lists (used for lexical input and for the name the
/// file system reports during the walk). `exempt` lifts the `.claude` / `.foreman` rule for the
/// typed exceptions.
///
/// # Errors
/// The deny rule that matched.
pub fn check_component(component: &str, exempt: Option<&str>) -> Result<(), PolicyViolation> {
    let k = key(component);
    if DENIED_COMPONENTS.contains(&k.as_str()) && exempt != Some(k.as_str()) {
        return Err(PolicyViolation::DeniedComponent(component.to_owned()));
    }
    if DENIED_NAMES.contains(&k.as_str()) {
        return Err(PolicyViolation::DeniedName(component.to_owned()));
    }
    Ok(())
}

fn exact_key_path(comps: &[&str]) -> Vec<String> {
    comps.iter().map(|c| key(c)).collect()
}

/// The write check (DR-160): syntax, then the deny rules, then the allowlist.
///
/// # Errors
/// The first rule that refuses the path.
pub fn check_write(rel: &str, purpose: WritePurpose) -> Result<RepoPath, PolicyViolation> {
    let comps = syntax(rel)?;
    let keys = exact_key_path(&comps);
    let exempt = match purpose {
        WritePurpose::CouncilSetup(i) => COUNCIL_AGENTS.get(i).and_then(|agent| {
            (keys == ["spec", ".claude", "agents", &format!("{agent}.md")]
                && comps[1] == ".claude"
                && comps[2] == "agents")
                .then_some(".claude")
        }),
        WritePurpose::DecisionFile(d) => (rel == d.path()).then_some(".foreman"),
        _ => None,
    };
    for c in &comps {
        check_component(c, exempt)?;
    }
    let last = keys.last().map_or("", String::as_str);
    if last == "claude.md" {
        let root_assembler = purpose == WritePurpose::AssemblerClaudeMd && comps == ["CLAUDE.md"];
        if !root_assembler {
            return Err(PolicyViolation::ClaudeMd);
        }
        return Ok(RepoPath(rel.to_owned()));
    }
    if exempt.is_some() {
        return Ok(RepoPath(rel.to_owned()));
    }
    match purpose {
        WritePurpose::CouncilSetup(_)
        | WritePurpose::DecisionFile(_)
        | WritePurpose::AssemblerClaudeMd => Err(PolicyViolation::NotAllowlisted),
        WritePurpose::Repo | WritePurpose::TemplateOut => {
            if comps.len() >= 2 && (comps[0] == "docs" || comps[0] == "spec") {
                Ok(RepoPath(rel.to_owned()))
            } else {
                Err(PolicyViolation::NotAllowlisted)
            }
        }
    }
}

/// The secret-file patterns (SR-50, DR-161) on folded keys.
#[allow(
    clippy::case_sensitive_file_extension_comparisons,
    reason = "keys are already case-folded"
)]
fn secret(comps: &[&str]) -> Option<String> {
    let keys = exact_key_path(comps);
    for (i, k) in keys.iter().enumerate() {
        if k == ".ssh" || k == ".aws" {
            return Some(comps[i].to_owned());
        }
        if k == ".config" && keys.get(i + 1).is_some_and(|n| n == "gcloud") {
            return Some(format!("{}/{}", comps[i], comps[i + 1]));
        }
        let hit = k.starts_with(".env")
            || k.ends_with(".pem")
            || k.ends_with(".key")
            || k.starts_with("id_")
            || k.ends_with(".p12")
            || k == ".npmrc"
            || k == ".netrc";
        if hit {
            return Some(comps[i].to_owned());
        }
    }
    None
}

/// True when a relative path matches a secret pattern (for listings and the watcher).
#[must_use]
pub fn is_secret(rel: &str) -> bool {
    secret(&rel.split('/').collect::<Vec<_>>()).is_some()
}

/// The read check (DR-161): the syntax rules plus the secret patterns. Reads may name protected
/// folders (`.foreman/` decision files are read), but never secrets.
///
/// # Errors
/// The first rule that refuses the path.
pub fn check_read(rel: &str, _class: ReadClass) -> Result<RepoPath, PolicyViolation> {
    let comps = syntax(rel)?;
    if let Some(s) = secret(&comps) {
        return Err(PolicyViolation::Secret(s));
    }
    Ok(RepoPath(rel.to_owned()))
}

/// An app-data-relative path (Foreman's own layout): the syntax rules only.
///
/// # Errors
/// The first syntax rule that refuses the path.
pub fn check_app_data(rel: &str) -> Result<RepoPath, PolicyViolation> {
    syntax(rel)?;
    Ok(RepoPath(rel.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folding() {
        assert_eq!(key(".GIT"), ".git");
        assert_eq!(key(".husKy"), ".husky", "Kelvin sign folds to k");
        assert_eq!(key("Cafe\u{301}"), key("Café"));
    }

    #[test]
    fn exceptions() {
        assert!(
            check_write(
                "spec/.claude/agents/red-team.md",
                WritePurpose::CouncilSetup(5)
            )
            .is_ok()
        );
        assert!(
            check_write(
                "spec/.claude/agents/red-team.md",
                WritePurpose::CouncilSetup(0)
            )
            .is_err()
        );
        assert!(
            check_write(
                ".foreman/project.json",
                WritePurpose::DecisionFile(DecisionPath::Project)
            )
            .is_ok()
        );
        assert!(
            check_write(
                ".foreman/overlay.json",
                WritePurpose::DecisionFile(DecisionPath::Project)
            )
            .is_err()
        );
        assert!(check_write("CLAUDE.md", WritePurpose::AssemblerClaudeMd).is_ok());
        assert_eq!(
            check_write("CLAUDE.md", WritePurpose::Repo),
            Err(PolicyViolation::ClaudeMd)
        );
    }
}
