//! The `fm:` grammar v1 (DR-020–DR-036): lexical form of directives and the value grammar of every
//! `fm:step` key. Hand-written, no parser generator (ADR-005).

use serde::{Deserialize, Serialize};

use super::expr::{Expr, parse_expr};
use crate::diagnostic::Diagnostic;
use crate::ids::{CwdKind, Effort, Mode, Model, Start, Surface, is_item_id, is_step_id};
use crate::pathpolicy::{self, WritePurpose};

/// The grammar version this Foreman reads.
pub const GRAMMAR_VERSION: u32 = 1;

/// A source position (1-based line, byte column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct SourcePos {
    /// Line.
    pub line: u32,
    /// Column.
    pub column: u32,
}

/// One `key=value` token of a directive with its position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// Key (or the whole token for `fm:if`).
    pub key: String,
    /// Value.
    pub value: String,
    /// Position of the token.
    pub pos: SourcePos,
}

/// A lexed directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Directive {
    /// `grammar`, `step`, `verify`, `if`, `end`, or an unknown name.
    pub name: String,
    /// The tokens after the name.
    pub tokens: Vec<Token>,
    /// Position of the comment.
    pub pos: SourcePos,
}

/// Step kinds (DR-022).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StepKind {
    /// A prompt (launch or re-arm).
    Prompt,
    /// Sent into the live session.
    Followup,
    /// A fixed Rust action.
    Action,
    /// Billy does it.
    Manual,
    /// Wait until a date.
    Wait,
    /// Runs any number of times.
    Repeatable,
}

impl StepKind {
    /// The annotation name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Prompt => "prompt",
            Self::Followup => "followup",
            Self::Action => "action",
            Self::Manual => "manual",
            Self::Wait => "wait",
            Self::Repeatable => "repeatable",
        }
    }

    /// Kinds that have a prompt and a run config.
    #[must_use]
    pub const fn is_prompt_kind(self) -> bool {
        matches!(self, Self::Prompt | Self::Followup | Self::Repeatable)
    }

    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "prompt" => Self::Prompt,
            "followup" => Self::Followup,
            "action" => Self::Action,
            "manual" => Self::Manual,
            "wait" => Self::Wait,
            "repeatable" => Self::Repeatable,
            _ => return None,
        })
    }
}

/// Tags (DR-022, MASTER-SPEC §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Tag {
    /// Never skipped.
    Core,
    /// Applies when `when` is true.
    Conditional,
    /// Skippable with a reason.
    Optional,
}

impl Tag {
    /// The annotation name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Conditional => "conditional",
            Self::Optional => "optional",
        }
    }
}

/// `new` or `continue`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Session {
    /// A new session or chat.
    New,
    /// Continues a chain.
    Continue,
}

/// `switch=<trigger>:<model>@<effort>/<mode>` (DR-105: a cue, never typed).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Switch {
    /// The trigger (`plan-approved`).
    pub trigger: String,
    /// The start after the switch.
    pub to: Start,
}

/// `pro-plan` / `max-plan` (DR-024).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AltRule {
    /// Pro plan.
    ProPlan,
    /// Max plan.
    MaxPlan,
}

impl AltRule {
    /// The annotation name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProPlan => "pro-plan",
            Self::MaxPlan => "max-plan",
        }
    }
}

/// `alt=<rule>:<model>@<effort>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Alt {
    /// Which plan selects it.
    pub rule: AltRule,
    /// Model.
    pub model: Model,
    /// Effort.
    pub effort: Effort,
}

/// `text` or `multiline`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InputType {
    /// One line.
    Text,
    /// Several lines.
    Multiline,
}

/// `in=<name>:<type>[:remember]` (DR-025).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputDecl {
    /// The input name (`paste_numbers`).
    pub name: String,
    /// Type.
    pub kind: InputType,
    /// Prefill from the project's last value.
    pub remember: bool,
}

/// `chain=` value.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChainRef {
    /// A step ID (`2.2`, `gate.A`).
    Step(String),
    /// `@stage` (F.2).
    Stage,
}

/// A parsed `fm:step` (values only; placement and cross-step rules live in `structure`/`validate`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepAnnotation {
    /// `id`
    pub id: String,
    /// `kind` (default prompt)
    pub kind: StepKind,
    /// `tag` (default core)
    pub tag: Tag,
    /// `when` source and parse
    pub when: Option<(String, Expr)>,
    /// `surface`
    pub surface: Option<Surface>,
    /// `cwd`
    pub cwd: Option<CwdKind>,
    /// `session` (default new)
    pub session: Session,
    /// `chain`
    pub chain: Option<ChainRef>,
    /// `start`
    pub start: Option<Start>,
    /// `switch` (repeatable)
    pub switches: Vec<Switch>,
    /// `alt` (repeatable)
    pub alts: Vec<Alt>,
    /// `in` (repeatable)
    pub inputs: Vec<InputDecl>,
    /// `out`
    pub out: Vec<String>,
    /// `appends`
    pub appends: Option<String>,
    /// `requires`
    pub requires: Vec<String>,
    /// Keys present, in source order.
    pub keys: Vec<String>,
    /// Keys this Foreman doesn't know (only kept when the grammar is newer, DR-035).
    pub unknown_keys: Vec<String>,
    /// Position of the comment.
    pub pos: SourcePos,
}

/// A `fm:verify` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyEntry {
    /// Item ID.
    pub id: String,
    /// Kind text (`attest`, `time:7d>attest`, `auto:cmd=x`).
    pub kind: String,
    /// Position.
    pub pos: SourcePos,
}

/// Lexes an HTML comment that starts with `<!-- fm:` (DR-020). `at` is the comment's position.
///
/// # Errors
/// A diagnostic for an unterminated comment or an empty name.
pub fn lex(literal: &str, at: SourcePos, file: &str) -> Result<Directive, Diagnostic> {
    let trimmed = literal.trim_end();
    let Some(body) = trimmed.strip_prefix("<!--").and_then(|b| b.strip_suffix("-->")) else {
        return Err(Diagnostic::error(file, at.line as usize, at.column as usize, "grammar.bad-directive", "an fm: annotation must be one HTML comment ending with -->"));
    };
    // Token positions: walk the literal tracking line and column (body starts 4 bytes in).
    let mut toks: Vec<(String, SourcePos)> = Vec::new();
    let mut line = at.line;
    let mut col = at.column + 4;
    let mut cur = String::new();
    let mut cur_pos = SourcePos { line, column: col };
    for ch in body.chars() {
        if ch.is_whitespace() {
            if !cur.is_empty() {
                toks.push((std::mem::take(&mut cur), cur_pos));
            }
            if ch == '\n' {
                line += 1;
                col = 1;
            } else {
                col += u32::try_from(ch.len_utf8()).unwrap_or(1);
            }
            continue;
        }
        if cur.is_empty() {
            cur_pos = SourcePos { line, column: col };
        }
        cur.push(ch);
        col += u32::try_from(ch.len_utf8()).unwrap_or(1);
    }
    if !cur.is_empty() {
        toks.push((cur, cur_pos));
    }
    let mut it = toks.into_iter();
    let Some((head, _)) = it.next() else {
        return Err(Diagnostic::error(file, at.line as usize, at.column as usize, "grammar.bad-directive", "empty annotation"));
    };
    let name = head.strip_prefix("fm:").unwrap_or(&head).to_owned();
    let tokens = it
        .map(|(t, pos)| match t.split_once('=') {
            Some((k, v)) => Token { key: k.to_owned(), value: v.to_owned(), pos },
            None => Token { key: t.clone(), value: String::new(), pos },
        })
        .collect();
    Ok(Directive { name, tokens, pos: at })
}

/// Parses `<model>@<effort>[/<mode>]` (DR-023). `bypassPermissions` is never a mode.
///
/// # Errors
/// `(rule, message)`.
pub fn parse_start(v: &str) -> Result<Start, (&'static str, String)> {
    let (me, mode) = match v.split_once('/') {
        Some((a, b)) => (a, Some(b)),
        None => (v, None),
    };
    let Some((m, e)) = me.split_once('@') else {
        return Err(("grammar.unknown-value", format!("`{v}` is not <model>@<effort>[/<mode>]")));
    };
    let model = Model::parse(m).ok_or(("grammar.unknown-value", format!("unknown model `{m}`")))?;
    let effort = Effort::parse(e).ok_or(("grammar.unknown-value", format!("unknown effort `{e}`")))?;
    let mode = match mode {
        None => None,
        Some(x) if x.eq_ignore_ascii_case("bypassPermissions") => {
            return Err(("grammar.bypass-permissions", "bypassPermissions is never allowed (SR-03)".into()));
        }
        Some(x) => Some(Mode::parse(x).ok_or(("grammar.unknown-value", format!("unknown mode `{x}`")))?),
    };
    Ok(Start { model, effort, mode })
}

fn parse_alt(v: &str) -> Result<Alt, (&'static str, String)> {
    let Some((rule, me)) = v.split_once(':') else {
        return Err(("grammar.unknown-value", format!("`{v}` is not <rule>:<model>@<effort>")));
    };
    let rule = match rule {
        "pro-plan" => AltRule::ProPlan,
        "max-plan" => AltRule::MaxPlan,
        other => return Err(("grammar.unknown-alt-rule", format!("unknown alternative rule `{other}`"))),
    };
    let s = parse_start(me)?;
    if s.mode.is_some() {
        return Err(("grammar.unknown-value", "an alternative has no mode".into()));
    }
    Ok(Alt { rule, model: s.model, effort: s.effort })
}

fn parse_input(v: &str) -> Result<InputDecl, (&'static str, String)> {
    let parts: Vec<&str> = v.split(':').collect();
    let name = parts.first().copied().unwrap_or_default();
    let ok_name = name.len() <= 64
        && name.starts_with(|c: char| c.is_ascii_alphabetic())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !ok_name {
        return Err(("grammar.unknown-value", format!("bad input name `{name}`")));
    }
    let kind = match parts.get(1) {
        Some(&"text") => InputType::Text,
        Some(&"multiline") => InputType::Multiline,
        _ => return Err(("grammar.unknown-value", format!("`{v}` is not <name>:<text|multiline>[:remember]"))),
    };
    let remember = match parts.get(2) {
        None => false,
        Some(&"remember") if parts.len() == 3 => true,
        _ => return Err(("grammar.unknown-value", format!("`{v}` has an unknown input option"))),
    };
    Ok(InputDecl { name: name.to_owned(), kind, remember })
}

/// Checks an annotation path (DR-026): repo-relative, through the write policy (TemplateOut) for
/// `out`/`appends`, the read policy for `requires`. `NN` in `stage-NN` is allowed in templates.
fn check_path(v: &str, write: bool) -> Result<(), (&'static str, String)> {
    let r = if write {
        pathpolicy::check_write(v, WritePurpose::TemplateOut).map(|_| ())
    } else {
        pathpolicy::check_read(v, pathpolicy::ReadClass::Decision).map(|_| ())
    };
    r.map_err(|e| ("grammar.path-policy", format!("`{v}`: {e} ({})", e.rule())))
}

/// The keys DR-022 allows, with whether each is repeatable.
const KEYS: [(&str, bool); 15] = [
    ("id", false),
    ("kind", false),
    ("tag", false),
    ("when", false),
    ("surface", false),
    ("cwd", false),
    ("session", false),
    ("chain", false),
    ("start", false),
    ("switch", true),
    ("alt", true),
    ("in", true),
    ("out", false),
    ("appends", false),
    ("requires", false),
];

fn allowed_on(key: &str, kind: StepKind) -> bool {
    match key {
        "id" | "kind" | "tag" | "when" | "requires" => true,
        "surface" | "cwd" | "session" | "chain" | "start" | "switch" | "alt" | "in" | "appends" => kind.is_prompt_kind(),
        "out" => kind.is_prompt_kind() || kind == StepKind::Manual,
        _ => false,
    }
}

/// Parses the tokens of an `fm:step` (DR-022). `grammar` is the declared version. Diagnostics are
/// appended; `None` when the annotation is unusable (no valid `id`).
pub fn parse_step(d: &Directive, grammar: u32, file: &str, diags: &mut Vec<Diagnostic>) -> Option<StepAnnotation> {
    let err = |diags: &mut Vec<Diagnostic>, pos: SourcePos, rule: &str, msg: String| {
        diags.push(Diagnostic::error(file, pos.line as usize, pos.column as usize, rule, msg));
    };
    let mut a = StepAnnotation {
        id: String::new(),
        kind: StepKind::Prompt,
        tag: Tag::Core,
        when: None,
        surface: None,
        cwd: None,
        session: Session::New,
        chain: None,
        start: None,
        switches: Vec::new(),
        alts: Vec::new(),
        inputs: Vec::new(),
        out: Vec::new(),
        appends: None,
        requires: Vec::new(),
        keys: Vec::new(),
        unknown_keys: Vec::new(),
        pos: d.pos,
    };
    // kind first: allowed-key checks depend on it.
    if let Some(t) = d.tokens.iter().find(|t| t.key == "kind") {
        match StepKind::parse(&t.value) {
            Some(k) => a.kind = k,
            None => err(diags, t.pos, "grammar.unknown-value", format!("unknown kind `{}`", t.value)),
        }
    }
    for t in &d.tokens {
        let Some((_, repeatable)) = KEYS.iter().find(|(k, _)| *k == t.key) else {
            if grammar > GRAMMAR_VERSION {
                a.unknown_keys.push(t.key.clone());
            } else {
                err(diags, t.pos, "grammar.unknown-key", format!("unknown key `{}` (grammar v{GRAMMAR_VERSION})", t.key));
            }
            continue;
        };
        if !repeatable && a.keys.contains(&t.key) {
            err(diags, t.pos, "grammar.duplicate-key", format!("`{}` appears twice", t.key));
            continue;
        }
        a.keys.push(t.key.clone());
        if !allowed_on(&t.key, a.kind) {
            err(diags, t.pos, "grammar.key-not-allowed", format!("`{}` isn't allowed on kind={}", t.key, a.kind.as_str()));
            continue;
        }
        if t.value.is_empty() {
            err(diags, t.pos, "grammar.unknown-value", format!("`{}` needs a value", t.key));
            continue;
        }
        let v = t.value.as_str();
        let r: Result<(), (&'static str, String)> = match t.key.as_str() {
            "id" => {
                if is_step_id(v) {
                    a.id = v.to_owned();
                    Ok(())
                } else {
                    Err(("grammar.bad-id", format!("`{v}` isn't a step ID")))
                }
            }
            "kind" => Ok(()),
            "tag" => match v {
                "core" => Ok(a.tag = Tag::Core),
                "conditional" => Ok(a.tag = Tag::Conditional),
                "optional" => Ok(a.tag = Tag::Optional),
                _ => Err(("grammar.unknown-value", format!("unknown tag `{v}`"))),
            },
            "when" => parse_expr(v)
                .map(|e| a.when = Some((v.to_owned(), e)))
                .map_err(|e| (e.rule(), e.to_string())),
            "surface" => match v {
                "chat" => Ok(a.surface = Some(Surface::Chat)),
                "cli" => Ok(a.surface = Some(Surface::Cli)),
                "desktop" => Ok(a.surface = Some(Surface::Desktop)),
                _ => Err(("grammar.unknown-value", format!("unknown surface `{v}`"))),
            },
            "cwd" => match v {
                "project" => Ok(a.cwd = Some(CwdKind::Project)),
                "spec" => Ok(a.cwd = Some(CwdKind::Spec)),
                "blueprint" | "worktree" => Err(("grammar.v1-1-cwd", format!("cwd={v} is v1.1"))),
                _ => Err(("grammar.unknown-value", format!("unknown cwd `{v}`"))),
            },
            "session" => match v {
                "new" => Ok(a.session = Session::New),
                "continue" => Ok(a.session = Session::Continue),
                _ => Err(("grammar.unknown-value", format!("unknown session `{v}`"))),
            },
            "chain" => {
                if v == "@stage" {
                    a.chain = Some(ChainRef::Stage);
                    Ok(())
                } else if is_step_id(v) {
                    a.chain = Some(ChainRef::Step(v.to_owned()));
                    Ok(())
                } else {
                    Err(("grammar.bad-chain", format!("`{v}` isn't a step ID or @stage")))
                }
            }
            "start" => parse_start(v).map(|s| a.start = Some(s)),
            "switch" => v
                .split_once(':')
                .ok_or(("grammar.unknown-value", format!("`{v}` is not <trigger>:<start>")))
                .and_then(|(trig, s)| {
                    let to = parse_start(s)?;
                    let ok = !trig.is_empty() && trig.bytes().all(|c| c.is_ascii_lowercase() || c == b'-');
                    if ok {
                        a.switches.push(Switch { trigger: trig.to_owned(), to });
                        Ok(())
                    } else {
                        Err(("grammar.unknown-value", format!("bad trigger `{trig}`")))
                    }
                }),
            "alt" => parse_alt(v).map(|x| a.alts.push(x)),
            "in" => parse_input(v).map(|x| a.inputs.push(x)),
            "out" => v.split(',').try_for_each(|p| check_path(p, true).map(|()| a.out.push(p.to_owned()))),
            "appends" => check_path(v, true).map(|()| a.appends = Some(v.to_owned())),
            "requires" => v.split(',').try_for_each(|p| check_path(p, false).map(|()| a.requires.push(p.to_owned()))),
            _ => Ok(()),
        };
        if let Err((rule, msg)) = r {
            err(diags, t.pos, rule, msg);
        }
    }
    if a.id.is_empty() {
        if !d.tokens.iter().any(|t| t.key == "id") {
            err(diags, d.pos, "grammar.missing-id", "fm:step needs an id".into());
        }
        return None;
    }
    Some(a)
}

/// Parses an `fm:verify` (DR-030, DR-031, DR-034).
pub fn parse_verify(d: &Directive, grammar: u32, file: &str, diags: &mut Vec<Diagnostic>) -> Vec<VerifyEntry> {
    let mut out = Vec::new();
    for t in &d.tokens {
        if !is_item_id(&t.key) {
            diags.push(Diagnostic::error(file, t.pos.line as usize, t.pos.column as usize, "grammar.bad-id", format!("`{}` isn't an item ID", t.key)));
            continue;
        }
        let kind_ok = t.value == "attest"
            || t.value.strip_prefix("time:").is_some_and(|r| {
                let (n, rest) = r.split_once('d').unwrap_or((r, "x"));
                n.parse::<u32>().is_ok_and(|n| (1..=365).contains(&n)) && !n.starts_with('0') && (rest.is_empty() || rest == ">attest")
            })
            || t.value.strip_prefix("auto:cmd=").is_some_and(is_item_id);
        if !kind_ok {
            let sev = if grammar > GRAMMAR_VERSION { "warning" } else { "error" };
            let msg = format!("unknown item kind `{}`", t.value);
            diags.push(if sev == "warning" {
                Diagnostic::warning(file, t.pos.line as usize, t.pos.column as usize, "grammar.newer-grammar", msg)
            } else {
                Diagnostic::error(file, t.pos.line as usize, t.pos.column as usize, "grammar.unknown-value", msg)
            });
        }
        if t.value.starts_with("auto:") {
            diags.push(Diagnostic::warning(file, t.pos.line as usize, t.pos.column as usize, "grammar.auto-needs-v1-1", "auto items need Foreman v1.1; treated as attest"));
        }
        out.push(VerifyEntry { id: t.key.clone(), kind: t.value.clone(), pos: t.pos });
    }
    out
}
