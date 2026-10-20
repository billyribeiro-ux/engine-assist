//! Cross-step validation, gate and F.2 instances, inheritance along chains, and the diagnostic
//! scoping that decides which steps are runnable (DR-011, DR-012, DR-022–DR-037, DR-053).
#![allow(
    clippy::too_many_lines,
    reason = "linear passes over the blueprint, kept whole for review against the domain rules"
)]

use std::collections::{BTreeMap, BTreeSet};

use super::grammar::{ChainRef, Session, StepKind, Tag};
use super::structure::Extras;
use super::{Step, StepStatus, Template};
use crate::diagnostic::Diagnostic;
use crate::ids::{Effort, StepKey, Surface, is_item_id};
use crate::pathpolicy::COUNCIL_AGENTS;
use crate::resolver::tokens::{self, ScanError, TokenClass};

/// The Rust action table (DR-036, SR-34): step ID → action.
pub const ACTIONS: [(&str, &str); 2] = [("council-setup", "write-blocks"), ("1.4b", "copy-file")];

struct V<'a> {
    file: &'a str,
    diags: Vec<Diagnostic>,
}

impl V<'_> {
    fn err(
        &mut self,
        line: u32,
        column: u32,
        stage: Option<u8>,
        rule: &str,
        msg: impl Into<String>,
    ) {
        self.diags.push(
            Diagnostic::error(self.file, line as usize, column as usize, rule, msg).in_stage(stage),
        );
    }

    fn warn(
        &mut self,
        line: u32,
        column: u32,
        stage: Option<u8>,
        rule: &str,
        msg: impl Into<String>,
    ) {
        self.diags.push(
            Diagnostic::warning(self.file, line as usize, column as usize, rule, msg)
                .in_stage(stage),
        );
    }
}

fn stage_of(s: &Step) -> Option<u8> {
    s.template_id.is_none().then_some(s.stage)
}

/// Per-step value rules that don't need other steps.
fn check_local(v: &mut V<'_>, s: &Step) {
    let Some(a) = &s.annotation else { return };
    let (line, col, st) = (a.pos.line, a.pos.column, stage_of(s));
    match (a.tag, a.when.is_some()) {
        (Tag::Conditional, false) => v.err(
            line,
            col,
            st,
            "grammar.when-required",
            format!("{}: tag=conditional needs when=", a.id),
        ),
        (Tag::Core, true) => v.err(
            line,
            col,
            st,
            "grammar.when-forbidden",
            format!("{}: tag=core can't have when=", a.id),
        ),
        _ => {}
    }
    if a.kind == StepKind::Action && !ACTIONS.iter().any(|(id, _)| *id == a.id) {
        v.err(
            line,
            col,
            st,
            "grammar.unknown-action",
            format!("no action is defined for step {}", a.id),
        );
    }
    if a.kind.is_prompt_kind() {
        if s.prompt.as_ref().is_none_or(|p| p.parts.is_empty()) {
            v.err(
                line,
                col,
                st,
                "grammar.missing-prompt",
                format!("{}: no `text` fence follows its fm:step", a.id),
            );
        }
    } else if s.prompt.is_some() && a.kind != StepKind::Action {
        // Non-prompt kinds have no prompt; a fence after them is display text only.
    }
}

/// Surface/mode/effort consistency once inheritance is known (DR-022, DR-023).
fn check_run_config(v: &mut V<'_>, s: &Step) {
    let Some(a) = &s.annotation else { return };
    if !a.kind.is_prompt_kind() {
        return;
    }
    let (line, col, st) = (a.pos.line, a.pos.column, stage_of(s));
    let Some(surface) = s.surface else {
        v.err(
            line,
            col,
            st,
            "grammar.missing-surface",
            format!("{}: surface is required", a.id),
        );
        return;
    };
    match surface {
        Surface::Chat => {
            if a.cwd.is_some() {
                v.err(
                    line,
                    col,
                    st,
                    "grammar.key-not-allowed",
                    format!("{}: cwd is only for cli/desktop", a.id),
                );
            }
            if !a.switches.is_empty() {
                v.err(
                    line,
                    col,
                    st,
                    "grammar.key-not-allowed",
                    format!("{}: switch is only for cli", a.id),
                );
            }
        }
        Surface::Cli | Surface::Desktop => {
            if s.cwd.is_none() {
                v.err(
                    line,
                    col,
                    st,
                    "grammar.missing-cwd",
                    format!("{}: cwd is required for {}", a.id, surface.as_str()),
                );
            }
            if surface == Surface::Desktop && !a.switches.is_empty() {
                v.err(
                    line,
                    col,
                    st,
                    "grammar.key-not-allowed",
                    format!("{}: switch is only for cli", a.id),
                );
            }
        }
    }
    let starts = s.start.iter().chain(a.switches.iter().map(|w| &w.to));
    for start in starts {
        match (surface, start.mode) {
            (Surface::Chat, Some(_)) => v.err(
                line,
                col,
                st,
                "grammar.mode-on-chat",
                format!("{}: chat steps have no mode", a.id),
            ),
            (Surface::Cli | Surface::Desktop, None) => v.err(
                line,
                col,
                st,
                "grammar.mode-required",
                format!("{}: cli/desktop steps need a mode", a.id),
            ),
            _ => {}
        }
        let adaptive = start.effort == Effort::Adaptive;
        if adaptive != (surface == Surface::Chat) {
            v.err(
                line,
                col,
                st,
                "grammar.effort-surface",
                format!(
                    "{}: effort `adaptive` is for chat only, and chat uses only `adaptive`",
                    a.id
                ),
            );
        }
    }
    for alt in &a.alts {
        if (alt.effort == Effort::Adaptive) != (surface == Surface::Chat) {
            v.err(
                line,
                col,
                st,
                "grammar.effort-surface",
                format!("{}: alternative effort doesn't fit the surface", a.id),
            );
        }
    }
    if s.start.is_none() && !(a.kind == StepKind::Followup && a.session == Session::Continue) {
        v.err(
            line,
            col,
            st,
            "grammar.missing-start",
            format!("{}: start is required", a.id),
        );
    }
}

/// Resolves chains and inheritance for a group of steps that chain to each other (one stage, or
/// the gate templates). `group` holds indices into `steps` in source order.
fn resolve_chains(v: &mut V<'_>, steps: &mut [Step], group: &[usize]) {
    for (pos, &i) in group.iter().enumerate() {
        let Some(a) = steps[i].annotation.clone() else {
            continue;
        };
        if !a.kind.is_prompt_kind() {
            continue;
        }
        let (line, col, st) = (a.pos.line, a.pos.column, stage_of(&steps[i]));
        match a.session {
            Session::New => {
                if a.chain.is_some() {
                    v.err(
                        line,
                        col,
                        st,
                        "grammar.chain-without-continue",
                        format!("{}: chain= needs session=continue", a.id),
                    );
                }
                steps[i].chain = Some(steps[i].key.clone());
            }
            Session::Continue => {
                let target = match &a.chain {
                    Some(ChainRef::Stage) => {
                        if a.id != "F.2" {
                            v.err(
                                line,
                                col,
                                st,
                                "grammar.stage-chain-outside-f2",
                                format!("{}: chain=@stage is only for F.2", a.id),
                            );
                        }
                        continue; // resolved per instance
                    }
                    Some(ChainRef::Step(id)) => group[..pos]
                        .iter()
                        .rev()
                        .find(|&&j| steps[j].id == *id)
                        .copied()
                        .or_else(|| {
                            let later = group[pos..].iter().any(|&j| steps[j].id == *id);
                            v.err(
                                line,
                                col,
                                st,
                                "grammar.bad-chain",
                                format!(
                                    "{}: chain={id} {}",
                                    a.id,
                                    if later {
                                        "is a later step"
                                    } else {
                                        "isn't an earlier step of this stage"
                                    }
                                ),
                            );
                            None
                        }),
                    None => {
                        let j = group[..pos]
                            .iter()
                            .rev()
                            .find(|&&j| {
                                steps[j]
                                    .annotation
                                    .as_ref()
                                    .is_some_and(|x| x.kind.is_prompt_kind())
                            })
                            .copied();
                        if let Some(j) = j {
                            v.warn(
                                line,
                                col,
                                st,
                                "lint.implicit-chain",
                                format!(
                                    "{}: session=continue without chain=; using {}",
                                    a.id, steps[j].id
                                ),
                            );
                        } else {
                            v.err(
                                line,
                                col,
                                st,
                                "grammar.bad-chain",
                                format!(
                                    "{}: session=continue with no earlier step in the stage",
                                    a.id
                                ),
                            );
                        }
                        j
                    }
                };
                let Some(j) = target else { continue };
                if !steps[j]
                    .annotation
                    .as_ref()
                    .is_some_and(|x| x.kind.is_prompt_kind())
                {
                    v.err(
                        line,
                        col,
                        st,
                        "grammar.bad-chain",
                        format!("{}: chain names a step that isn't a prompt", a.id),
                    );
                    continue;
                }
                let root = steps[j]
                    .chain
                    .clone()
                    .unwrap_or_else(|| steps[j].key.clone());
                let (rs, rc) = (steps[j].surface, steps[j].cwd);
                if let (Some(declared), Some(inherited)) = (a.surface, rs)
                    && declared != inherited
                {
                    v.err(
                        line,
                        col,
                        st,
                        "grammar.chain-surface-mismatch",
                        format!(
                            "{}: surface={} but its chain is {}",
                            a.id,
                            declared.as_str(),
                            inherited.as_str()
                        ),
                    );
                }
                if let (Some(declared), Some(inherited)) = (a.cwd, rc)
                    && declared != inherited
                {
                    v.err(
                        line,
                        col,
                        st,
                        "grammar.chain-cwd-mismatch",
                        format!(
                            "{}: cwd={} but its chain runs in {}",
                            a.id,
                            declared.as_str(),
                            inherited.as_str()
                        ),
                    );
                }
                steps[i].surface = rs.or(a.surface);
                steps[i].cwd = rc.or(a.cwd);
                // A followup without a start uses its chain's live start (DR-068).
                if a.kind == StepKind::Followup && a.start.is_none() {
                    let prev = group[..pos]
                        .iter()
                        .rev()
                        .find(|&&k| steps[k].chain.as_ref() == Some(&root))
                        .copied();
                    steps[i].start = prev.and_then(|k| live_start(&steps[k]));
                }
                steps[i].chain = Some(root);
            }
        }
    }
}

/// The live start of a step after its switches (DR-042).
fn live_start(s: &Step) -> Option<crate::ids::Start> {
    s.switches().last().map(|w| w.to).or(s.start)
}

#[allow(
    clippy::many_single_char_names,
    reason = "v/s/a/t/c follow the module's naming"
)]
fn check_tokens(v: &mut V<'_>, s: &Step) {
    let Some(a) = &s.annotation else { return };
    let Some(pr) = &s.prompt else { return };
    let declared: Vec<String> = a.inputs.iter().map(|i| i.name.clone()).collect();
    let st = stage_of(s);
    let mut found_inputs = BTreeSet::new();
    for part in &pr.parts {
        let (toks, errs) = tokens::scan(&part.text, &declared);
        for e in errs {
            let (rule, msg, off) = match e {
                ScanError::NestedOptional(o) => (
                    "grammar.nested-optional",
                    "[optional: inside an optional part",
                    o,
                ),
                ScanError::UnclosedOptional(o) => (
                    "grammar.unclosed-optional",
                    "[optional: without its closing ]",
                    o,
                ),
            };
            let (l, c) = line_col(&part.text, off);
            v.err(part.line + l, c, st, rule, format!("{}: {msg}", a.id));
        }
        for t in toks {
            let (l, c) = line_col(&part.text, t.offset);
            match t.class {
                None => v.err(
                    part.line + l,
                    c,
                    st,
                    "resolver.unknown-token",
                    format!("{}: unknown token {}", a.id, t.text),
                ),
                Some(TokenClass::RunInput) if t.inner != "N" => {
                    let name = tokens::input_name(&t.inner);
                    if !declared.contains(&name) {
                        v.err(
                            part.line + l,
                            c,
                            st,
                            "grammar.input-binding",
                            format!("{}: {} needs in={name}:<type>", a.id, t.text),
                        );
                    }
                    found_inputs.insert(name);
                }
                _ => {}
            }
        }
    }
    for d in &a.inputs {
        if !found_inputs.contains(&d.name) {
            v.err(
                a.pos.line,
                a.pos.column,
                st,
                "grammar.input-binding",
                format!(
                    "{}: in={} has no [{}] token in the prompt",
                    a.id,
                    d.name,
                    d.name.replace('_', " ")
                ),
            );
        }
    }
}

/// 0-based line offset and 1-based column of a byte offset in text.
pub(crate) fn line_col(text: &str, off: usize) -> (u32, u32) {
    let before = &text[..off.min(text.len())];
    let l = before.matches('\n').count();
    let c = before
        .rfind('\n')
        .map_or(before.len(), |p| before.len() - p - 1)
        + 1;
    (u32::try_from(l).unwrap_or(0), u32::try_from(c).unwrap_or(1))
}

fn instantiate_out(paths: &[String], stage: u8) -> Vec<String> {
    paths
        .iter()
        .map(|p| p.replace("stage-NN", &format!("stage-{stage:02}")))
        .collect()
}

fn instance(tpl: &Step, stage: u8) -> Step {
    let id = tpl.template_id.clone().unwrap_or_default();
    let mut s = tpl.clone();
    s.key = StepKey::instance(stage, &id);
    s.stage = stage;
    s.out = instantiate_out(&tpl.out, stage);
    s.appends = tpl
        .appends
        .as_ref()
        .map(|a| a.replace("stage-NN", &format!("stage-{stage:02}")));
    s.chain = tpl.chain.as_ref().map(|c| {
        let cid = c.as_str().split_once(':').map_or(c.as_str(), |(_, x)| x);
        StepKey::instance(stage, cid)
    });
    s
}

/// The packet path of a stage.
#[must_use]
pub fn packet_path(stage: u8) -> String {
    format!("docs/handoff/stage-{stage:02}.md")
}

/// Validates templates and stage steps, creates the instances and resolves inheritance.
pub(super) fn run(t: &mut Template, extras: &Extras, file: &str) {
    let mut v = V {
        file,
        diags: Vec::new(),
    };
    // Gate templates chain to each other.
    let gidx: Vec<usize> = (0..t.gate_templates.len()).collect();
    resolve_chains(&mut v, &mut t.gate_templates, &gidx);
    for s in &t.gate_templates {
        check_local(&mut v, s);
        check_run_config(&mut v, s);
        check_tokens(&mut v, s);
    }
    if let Some(f2) = &t.f2_template {
        check_local(&mut v, f2);
        check_tokens(&mut v, f2);
        if let Some(a) = &f2.annotation {
            if a.chain != Some(ChainRef::Stage) || a.session != Session::Continue {
                v.err(
                    a.pos.line,
                    a.pos.column,
                    None,
                    "grammar.bad-chain",
                    "F.2 must be session=continue chain=@stage",
                );
            }
            if a.surface.is_some() || a.cwd.is_some() || a.start.is_some() {
                v.err(
                    a.pos.line,
                    a.pos.column,
                    None,
                    "grammar.stage-chain-keys",
                    "a chain=@stage step must omit surface, cwd and start",
                );
            }
        }
    }
    // Stage steps, grouped by stage.
    let stages: Vec<u8> = t.stages.iter().map(|s| s.number).collect();
    for &n in &stages {
        let group: Vec<usize> = t
            .steps
            .iter()
            .enumerate()
            .filter(|(_, s)| s.stage == n)
            .map(|(i, _)| i)
            .collect();
        resolve_chains(&mut v, &mut t.steps, &group);
    }
    for s in &t.steps {
        check_local(&mut v, s);
        check_run_config(&mut v, s);
        check_tokens(&mut v, s);
    }
    // Duplicate step IDs (template steps) and item IDs (DR-005, DR-034).
    let mut seen: BTreeMap<String, u32> = BTreeMap::new();
    for s in t
        .steps
        .iter()
        .chain(t.gate_templates.iter())
        .chain(t.f2_template.iter())
    {
        if let Some(a) = &s.annotation
            && seen.insert(a.id.clone(), a.pos.line).is_some()
        {
            v.err(
                a.pos.line,
                a.pos.column,
                stage_of(s),
                "grammar.duplicate-id",
                format!("step ID {} is used twice", a.id),
            );
        }
    }
    let mut items: BTreeSet<String> = BTreeSet::new();
    for it in &t.items {
        if let Some(id) = &it.id
            && (!is_item_id(id) || !items.insert(id.clone()))
        {
            v.err(
                it.line,
                1,
                Some(it.stage),
                "grammar.duplicate-id",
                format!("item ID {id} is used twice"),
            );
        }
    }
    // Council blocks (DR-036): every fence names a known agent.
    for b in &t.council_blocks {
        if !COUNCIL_AGENTS.contains(&b.name.as_str()) {
            v.err(
                b.line,
                1,
                Some(2),
                "grammar.unknown-action-target",
                format!("council-setup: `{}` isn't a council agent", b.name),
            );
        }
    }
    // Instances: gate (5–17) and F.2 (0–4).
    let mut inserts: Vec<(usize, Step)> = Vec::new();
    for &(stage, at) in &extras.gate_positions {
        for tpl in &t.gate_templates {
            inserts.push((at, instance(tpl, stage)));
        }
    }
    if let Some(f2) = t.f2_template.clone() {
        for &n in stages.iter().filter(|n| **n <= 4) {
            let pkt = packet_path(n);
            let producer = t
                .steps
                .iter()
                .rposition(|s| s.stage == n && s.out.contains(&pkt));
            let last = t.steps.iter().rposition(|s| s.stage == n);
            let Some(at) = producer.or(last) else {
                continue;
            };
            let mut inst = instance(&f2, n);
            if let Some(p) = producer.map(|i| &t.steps[i]) {
                inst.chain = p.chain.clone().or_else(|| Some(p.key.clone()));
                inst.surface = p.surface;
                inst.cwd = p.cwd;
                inst.start = live_start(p);
            }
            inserts.push((at + 1, inst));
        }
    }
    inserts.sort_by_key(|(at, _)| *at);
    for (k, (at, s)) in inserts.into_iter().enumerate() {
        t.steps.insert(at + k, s);
    }
    // Packet producer (DR-053): exactly one per annotated stage 0–4.
    for &n in stages.iter().filter(|n| **n <= 4) {
        let annotated = t
            .steps
            .iter()
            .any(|s| s.stage == n && s.annotation.is_some() && s.template_id.is_none());
        if !annotated {
            continue;
        }
        let pkt = packet_path(n);
        let count = t
            .steps
            .iter()
            .filter(|s| s.stage == n && s.out.contains(&pkt))
            .count();
        if count != 1 {
            let line = t
                .stages
                .iter()
                .find(|s| s.number == n)
                .map_or(1, |s| s.line);
            v.err(
                line,
                1,
                Some(n),
                "grammar.packet-producer",
                format!("Stage {n} needs exactly one step with out={pkt} (found {count})"),
            );
        }
    }
    t.diagnostics.extend(v.diags);
}

/// Marks steps not runnable when their stage (or their template) has an error (DR-037, AC-F001-15).
pub(super) fn finish(t: &mut Template, extras: &Extras) {
    let errors: Vec<&Diagnostic> = t.diagnostics.iter().filter(|d| d.is_error()).collect();
    let global = errors.iter().any(|d| {
        matches!(
            d.rule.as_str(),
            "grammar.missing-version" | "structure.depth-limit"
        )
    }) || t.grammar_version.is_none();
    let in_range = |line: u32, r: &(u32, u32)| line >= r.0 && line < r.1;
    let mut bad_stages: BTreeSet<u8> = BTreeSet::new();
    let mut bad_templates = false;
    let mut everything = global;
    for d in &errors {
        let stage = d.stage.or_else(|| {
            extras
                .stage_ranges
                .iter()
                .find(|(_, a, b)| d.line >= *a && d.line < *b)
                .map(|(n, _, _)| *n)
        });
        if let Some(n) = stage {
            bad_stages.insert(n);
        } else if extras.template_ranges.iter().any(|r| in_range(d.line, r)) {
            bad_templates = true;
        } else {
            everything = true;
        }
    }
    for s in &mut t.steps {
        if s.status != StepStatus::Runnable {
            continue;
        }
        if everything || bad_stages.contains(&s.stage) || (bad_templates && s.template_id.is_some())
        {
            s.status = StepStatus::Invalid;
        }
    }
}
