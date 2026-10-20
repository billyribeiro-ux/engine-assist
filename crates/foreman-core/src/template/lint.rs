//! Prose lint (DR-040–DR-043): the r2 P-01 inference rules run over RUN WITH text (and prompt text
//! for outputs) and produce `lint.*` warnings when they disagree with the annotation. They never
//! supply or change a value (R-05). Chat chips are memory aids from the same prose.

use super::grammar::{Session, StepKind};
use super::{Step, Template};
use crate::diagnostic::Diagnostic;
use crate::ids::{CwdKind, Effort, Mode, Model};

const MODES: [(&str, Mode); 5] = [
    ("**Plan**", Mode::Plan),
    ("**Manual**", Mode::Default),
    ("**Accept edits**", Mode::AcceptEdits),
    ("**Auto**", Mode::Auto),
    ("**dontAsk**", Mode::DontAsk),
];

const MODELS: [(&str, Model); 5] = [
    ("**Fable 5.1**", Model::Fable),
    ("**Opus 5.5**", Model::Opus),
    ("**Sonnet 5.5**", Model::Sonnet),
    ("**Haiku 5.5**", Model::Haiku),
    ("**opusplan**", Model::Opusplan),
];

/// What the prose implies (each field only when the prose says it).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Inferred {
    /// Rule 1.
    pub session: Option<Session>,
    /// Rule 2.
    pub cwd: Option<CwdKind>,
    /// Rule 3: start mode.
    pub mode: Option<Mode>,
    /// Rule 3: switch target.
    pub switch_to: Option<Mode>,
    /// Rule 4.
    pub model: Option<Model>,
    /// Rule 4.
    pub effort: Option<Effort>,
    /// Rule 5.
    pub has_alternative: bool,
    /// Rule 6: repo-relative outputs.
    pub outs: Vec<String>,
}

fn first_of<T: Copy>(text: &str, table: &[(&str, T)]) -> Vec<(usize, T)> {
    let mut v: Vec<(usize, T)> = table
        .iter()
        .flat_map(|(pat, val)| text.match_indices(pat).map(move |(i, _)| (i, *val)))
        .collect();
    v.sort_by_key(|(i, _)| *i);
    v
}

fn word_after<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let i = text.find(prefix)? + prefix.len();
    let rest = text[i..].trim_start_matches(['*', ' ', '`']);
    let end = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '-')).unwrap_or(rest.len());
    Some(&rest[..end])
}

fn resolve_out(path: &str, cwd: Option<CwdKind>) -> String {
    let p = path
        .trim_matches(|c: char| c == '`' || c == '.' || c == ',' || c == ':' || c == ';')
        .replace("[app]-spec/", "spec/")
        .replace("[N]", "NN");
    if let Some(up) = p.strip_prefix("../") {
        return up.to_owned();
    }
    match cwd {
        Some(CwdKind::Spec) if !p.starts_with("spec/") => format!("spec/{p}"),
        _ => p,
    }
}

/// Applies the six inference rules (DR-041).
#[must_use]
pub fn infer(run_with: &str, prompt: &str, cwd: Option<CwdKind>) -> Inferred {
    let lower = run_with.to_lowercase();
    let mut inf = Inferred::default();
    if lower.contains("same session") || lower.contains("same chat") {
        inf.session = Some(Session::Continue);
    }
    if ["fresh session", "new session", "new conversation", "brand-new conversation"]
        .iter()
        .any(|p| lower.contains(p))
    {
        inf.session = Some(Session::New);
    }
    if run_with.contains("in `[app]-spec/`") || run_with.contains("in `spec/`") {
        inf.cwd = Some(CwdKind::Spec);
    }
    let modes = first_of(run_with, &MODES);
    match modes.as_slice() {
        [] => {}
        [(i, m)] => {
            let _ = i;
            inf.mode = Some(*m);
        }
        [(i, a), (j, b), ..] => {
            let between = &run_with[*i..*j];
            let before = &run_with[..*i];
            if (between.contains("first, then") || between.contains('→')) && !before.trim_end().ends_with("Shift+Tab →") {
                inf.mode = Some(*a);
                inf.switch_to = Some(*b);
            } else {
                inf.mode = Some(*a);
            }
        }
    }
    if let Some(m) = word_after(run_with, "/model ") {
        inf.model = Model::parse(m);
    } else if let Some((_, m)) = first_of(run_with, &MODELS).first() {
        inf.model = Some(*m);
    }
    if let Some(e) = word_after(run_with, "/effort ") {
        inf.effort = Effort::parse(e);
    } else if let Some(e) = word_after(run_with, "effort ") {
        inf.effort = Effort::parse(e);
    }
    inf.has_alternative = run_with.contains("— or") || run_with.contains("(or ") || run_with.contains("if available, otherwise");
    for line in prompt.lines() {
        for (i, _) in line.match_indices("-> ") {
            if let Some(p) = line[i + 3..].split_whitespace().next() {
                inf.outs.push(resolve_out(p, cwd));
            }
        }
        for (i, _) in line.match_indices("Write ") {
            if let Some(p) = line[i + 6..].split_whitespace().next() {
                if p.contains('/') && p.contains('.') {
                    inf.outs.push(resolve_out(p, cwd));
                }
            }
        }
    }
    inf
}

fn lint_step(t: &Template, s: &Step, file: &str, out: &mut Vec<Diagnostic>) {
    let Some(a) = &s.annotation else { return };
    if !a.kind.is_prompt_kind() {
        return;
    }
    let st = s.template_id.is_none().then_some(s.stage);
    let mut warn = |line: u32, rule: &str, msg: String| {
        out.push(Diagnostic::warning(file, line as usize, 1, rule, msg).in_stage(st));
    };
    let prompt = s.prompt.as_ref().map(super::Prompt::joined).unwrap_or_default();
    if let Some(rw) = &s.run_with {
        let inf = infer(&rw.text, &prompt, s.cwd);
        let line = rw.line;
        if let Some(sess) = inf.session {
            if sess != a.session {
                warn(line, "lint.session-disagrees", format!("{}: the RUN WITH says {} session, the annotation says session={}", a.id, if sess == Session::New { "a new" } else { "the same" }, if a.session == Session::New { "new" } else { "continue" }));
            }
        }
        if let Some(c) = inf.cwd {
            if s.cwd != Some(c) {
                warn(line, "lint.cwd-disagrees", format!("{}: the RUN WITH says cwd {}, the annotation says {:?}", a.id, c.as_str(), s.cwd.map(CwdKind::as_str)));
            }
        }
        if let (Some(m), Some(start)) = (inf.mode, a.start) {
            if start.mode != Some(m) {
                warn(line, "lint.mode-disagrees", format!("{}: the RUN WITH says mode {}, the annotation says {}", a.id, m.as_str(), start));
            }
        }
        if let Some(m) = inf.switch_to {
            if a.switches.last().and_then(|w| w.to.mode) != Some(m) {
                warn(line, "lint.switch-disagrees", format!("{}: the RUN WITH switches to {}, the annotation has no such switch", a.id, m.as_str()));
            }
        }
        if let (Some(m), Some(start)) = (inf.model, a.start) {
            if start.model != m {
                warn(line, "lint.model-disagrees", format!("{}: the RUN WITH says model {}, the annotation says {}", a.id, m.as_str(), start.model.as_str()));
            }
        }
        if let (Some(e), Some(start)) = (inf.effort, a.start) {
            if start.effort != e {
                warn(line, "lint.effort-disagrees", format!("{}: the RUN WITH says effort {}, the annotation says {}", a.id, e.as_str(), start.effort.as_str()));
            }
        }
        if inf.has_alternative && a.alts.is_empty() {
            warn(line, "lint.alt-missing", format!("{}: the RUN WITH names an alternative; the annotation has no alt= (judgment alternatives are substitutions or overrides, DR-024)", a.id));
        }
        for o in &inf.outs {
            let declared: Vec<&String> = a.out.iter().chain(a.appends.iter()).collect();
            if !declared.iter().any(|d| *d == o) {
                warn(line, "lint.out-disagrees", format!("{}: the prompt writes {o}, which isn't in out=", a.id));
            }
        }
    }
    // DR-042: a followup's start equals its chain's live start.
    if a.kind == StepKind::Followup {
        if let (Some(start), Some(chain)) = (a.start, &s.chain) {
            let prev = t
                .steps
                .iter()
                .chain(t.gate_templates.iter())
                .take_while(|x| x.key != s.key)
                .filter(|x| x.stage == s.stage && x.template_id.is_some() == s.template_id.is_some() && x.chain.as_ref() == Some(chain))
                .last();
            if let Some(p) = prev {
                let live = p.switches().last().map(|w| w.to).or(p.start);
                let same = live.is_some_and(|l| l.model == start.model && l.effort == start.effort && l.mode == start.mode);
                if !same {
                    let line = a.pos.line;
                    out.push(Diagnostic::warning(file, line as usize, a.pos.column as usize, "lint.followup-changes-start", format!("{}: a followup can't change model, effort or mode; use kind=prompt to re-arm", a.id)).in_stage(st));
                }
            }
        }
    }
}

/// Runs the lint over every template step and gate/F.2 template.
pub(super) fn run(t: &mut Template, file: &str) {
    let mut out = Vec::new();
    for s in t.steps.iter().filter(|s| s.template_id.is_none()).chain(t.gate_templates.iter()).chain(t.f2_template.iter()) {
        lint_step(t, s, file, &mut out);
    }
    t.diagnostics.extend(out);
}

/// Chat setup chips (DR-043), from prose only. `project` is the project's display name.
#[must_use]
pub fn chat_chips(t: &Template, s: &Step, project: &str) -> Vec<String> {
    let Some(rw) = &s.run_with else { return Vec::new() };
    let mut chips = Vec::new();
    match s.session {
        Session::New => chips.push(format!("New conversation in Project {project}")),
        Session::Continue => {
            let start = s.chain.as_ref().and_then(|k| t.steps.iter().find(|x| &x.key == k));
            chips.push(format!("Same chat as {}", start.map_or_else(|| "the previous step".to_owned(), |x| format!("{} {}", x.id, x.title))));
        }
    }
    if let Some((i, _)) = first_of(&rw.text, &MODELS).first() {
        let name = rw.text[*i + 2..].split("**").next().unwrap_or_default();
        chips.push(name.to_owned());
    }
    for (needle, chip) in [("web search ON", "web search ON"), ("Research ON", "Research ON")] {
        if rw.text.contains(needle) {
            chips.push(chip.to_owned());
        }
    }
    chips
}
