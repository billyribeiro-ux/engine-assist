//! The single walk over comrak's AST (ADR-005): stages, steps, RUN WITH, prompts, inspection lists,
//! HANDOFF notes, packet formats and directive placement (DR-002–DR-017, DR-020, DR-030, DR-032).
#![allow(
    clippy::too_many_lines,
    reason = "linear passes over the blueprint, kept whole for review against the domain rules"
)]

use std::collections::BTreeSet;

use comrak::nodes::{AstNode, NodeValue};
use comrak::{Arena, Options};

use super::grammar::{self, Directive, SourcePos, StepAnnotation, StepKind, VerifyEntry};
use super::{
    CouncilBlock, FormatKey, HandoffNote, InspectionItem, PARSER_VERSION, PacketFormat, Prompt,
    PromptPart, RunWith, SOFT_LINE_LIMIT, Stage, Step, StepStatus, Template, collapse, item_hash,
    normalize_source, source_sha256,
};
use crate::diagnostic::Diagnostic;
use crate::ids::StepKey;

/// Container nesting limit (DR-015).
const MAX_DEPTH: usize = 64;

type Node<'a> = &'a AstNode<'a>;

fn pos(n: Node<'_>) -> SourcePos {
    let sp = n.data().sourcepos;
    SourcePos {
        line: u32::try_from(sp.start.line).unwrap_or(u32::MAX),
        column: u32::try_from(sp.start.column).unwrap_or(u32::MAX),
    }
}

fn end_line(n: Node<'_>) -> usize {
    n.data().sourcepos.end.line
}

/// Concatenated inline literal text (DR-014 rules), iteratively (no recursion on hostile input).
pub(crate) fn inline_text<'a>(n: Node<'a>, out: &mut String) {
    let mut stack: Vec<Node<'a>> = n.reverse_children().collect();
    while let Some(c) = stack.pop() {
        match &c.data().value {
            NodeValue::Text(t) => out.push_str(t),
            NodeValue::Code(code) => out.push_str(&code.literal),
            NodeValue::SoftBreak | NodeValue::LineBreak => out.push(' '),
            NodeValue::HtmlInline(h) => out.push_str(h),
            NodeValue::Paragraph if !out.is_empty() => out.push(' '),
            _ => {}
        }
        stack.extend(c.reverse_children());
    }
}

fn first_para(n: Node<'_>) -> Option<Node<'_>> {
    n.children()
        .find(|c| matches!(c.data().value, NodeValue::Paragraph))
}

/// The leading strong text of a blockquote's first paragraph (`RUN WITH`, `INSPECTION: …`, `HANDOFF`).
fn quote_label(n: Node<'_>) -> Option<String> {
    let p = first_para(n)?;
    let first = p.first_child()?;
    if !matches!(first.data().value, NodeValue::Strong) {
        return None;
    }
    let mut s = String::new();
    inline_text(first, &mut s);
    Some(s.trim().to_owned())
}

/// The source text of a node's lines, with `levels` blockquote markers stripped.
fn source_lines(lines: &[&str], n: Node<'_>, levels: usize) -> String {
    let sp = n.data().sourcepos;
    let mut out = Vec::new();
    for l in sp.start.line..=sp.end.line {
        let mut s = lines.get(l.saturating_sub(1)).copied().unwrap_or_default();
        for _ in 0..levels {
            s = s.trim_start();
            s = s.strip_prefix('>').unwrap_or(s);
            s = s.strip_prefix(' ').unwrap_or(s);
        }
        out.push(s);
    }
    out.join("\n")
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum Section {
    None,
    Stage(u8),
    Gate,
    AppendixF,
}

#[allow(clippy::struct_excessive_bools, reason = "walker state flags")]
struct Walker<'s> {
    file: &'s str,
    lines: Vec<&'s str>,
    t: Template,
    section: Section,
    /// Index of the current step in `t.steps` / gate templates / f2.
    cur: Option<Cur>,
    /// The prompt being collected is open (DR-008 continuation).
    prompt_open: bool,
    /// An `fm:if` part is open: (expr src, expr, line).
    open_if: Option<(String, super::Expr, u32)>,
    grammar_seen: bool,
    first_html_seen: bool,
    gate_positions: Vec<(u8, usize)>,
    in_gate_heading: Option<u8>,
    council: bool,
    stage_numbers: BTreeSet<u8>,
    /// (section, first line) whenever the section changes.
    marks: Vec<(Section, u32)>,
}

/// Facts the later passes need besides the template.
#[derive(Debug, Default)]
pub(super) struct Extras {
    /// (stage, index in `steps`) where each `### Stage N gate` heading sits.
    pub gate_positions: Vec<(u8, usize)>,
    /// Line ranges `[start, end)` of each stage.
    pub stage_ranges: Vec<(u8, u32, u32)>,
    /// Line ranges of the Stage Gate section and of Appendix F (templates).
    pub template_ranges: Vec<(u32, u32)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Cur {
    Step(usize),
    Gate(usize),
    F2,
}

fn heading_text(n: Node<'_>) -> String {
    let mut s = String::new();
    inline_text(n, &mut s);
    s.trim().to_owned()
}

fn new_step(key: &str, id: &str, stage: u8, title: &str, line: u32) -> Step {
    Step {
        key: StepKey::parse(key).unwrap_or_else(|_| StepKey::instance(stage, "F.2")),
        id: id.to_owned(),
        stage,
        title: title.to_owned(),
        line,
        status: StepStatus::NeedsAnnotation,
        annotation: None,
        kind: StepKind::Prompt,
        tag: super::Tag::Core,
        surface: None,
        cwd: None,
        session: super::Session::New,
        chain: None,
        start: None,
        out: Vec::new(),
        appends: None,
        run_with: None,
        prompt: None,
        template_id: None,
    }
}

impl Walker<'_> {
    fn err(&mut self, p: SourcePos, rule: &str, msg: impl Into<String>) {
        let stage = match self.section {
            Section::Stage(n) => Some(n),
            _ => None,
        };
        self.t.diagnostics.push(
            Diagnostic::error(self.file, p.line as usize, p.column as usize, rule, msg)
                .in_stage(stage),
        );
    }

    fn warn(&mut self, p: SourcePos, rule: &str, msg: impl Into<String>) {
        let stage = match self.section {
            Section::Stage(n) => Some(n),
            _ => None,
        };
        self.t.diagnostics.push(
            Diagnostic::warning(self.file, p.line as usize, p.column as usize, rule, msg)
                .in_stage(stage),
        );
    }

    fn cur_step(&mut self) -> Option<&mut Step> {
        match self.cur? {
            Cur::Step(i) => self.t.steps.get_mut(i),
            Cur::Gate(i) => self.t.gate_templates.get_mut(i),
            Cur::F2 => self.t.f2_template.as_mut(),
        }
    }

    fn close_prompt(&mut self, at: SourcePos) {
        if let Some((src, _, line)) = self.open_if.take() {
            self.err(
                SourcePos { line, column: 1 },
                "grammar.unmatched-if",
                format!("fm:if {src} has no fm:end before the prompt ends"),
            );
            let _ = at;
        }
        self.prompt_open = false;
    }

    fn heading(&mut self, n: Node<'_>, level: u8) {
        let text = heading_text(n);
        let p = pos(n);
        self.close_prompt(p);
        self.in_gate_heading = None;
        self.council = false;
        if level <= 2 {
            self.cur = None;
        }
        let before = self.section;
        self.heading_inner(&text, p, level);
        if level <= 2 && (self.section != before || level == 2) {
            self.marks.push((self.section, p.line));
        }
    }

    fn heading_inner(&mut self, text: &str, p: SourcePos, level: u8) {
        match level {
            1 => {
                if p.line > 1 {
                    self.section = Section::None;
                }
            }
            2 => {
                if let Some(rest) = text.strip_prefix("Stage ") {
                    let (num, title) = rest.split_once(':').unwrap_or((rest, ""));
                    if let Ok(nn) = num.trim().parse::<u8>() {
                        if nn > 18 {
                            self.err(
                                p,
                                "structure.stage-number",
                                format!("Stage {nn} is outside 0–18"),
                            );
                            self.section = Section::None;
                            return;
                        }
                        let last = self.t.stages.last().map(|s| s.number);
                        if self.stage_numbers.contains(&nn) || last.is_some_and(|l| nn < l) {
                            self.err(
                                p,
                                "structure.stage-order",
                                format!("Stage {nn} is out of order or repeated"),
                            );
                        }
                        self.stage_numbers.insert(nn);
                        self.t.stages.push(Stage {
                            number: nn,
                            title: title.trim().to_owned(),
                            line: p.line,
                            has_gate: false,
                        });
                        self.section = Section::Stage(nn);
                        return;
                    }
                }
                self.section = if text.starts_with("The Stage Gate") {
                    Section::Gate
                } else if text.starts_with("Appendix F") {
                    Section::AppendixF
                } else {
                    Section::None
                };
            }
            3 => self.heading3(text, p),
            _ => {}
        }
    }

    fn heading3(&mut self, text: &str, p: SourcePos) {
        self.cur = None;
        match self.section {
            Section::Stage(stage) => {
                if let Some(rest) = text.strip_prefix("Prompt ") {
                    let (num, title) = rest.split_once(':').unwrap_or((rest, ""));
                    let num = num.trim();
                    let stage_of = num.split('.').next().and_then(|x| x.parse::<u8>().ok());
                    if stage_of != Some(stage) {
                        self.err(
                            p,
                            "structure.step-stage-mismatch",
                            format!("Prompt {num} is under Stage {stage}"),
                        );
                        return;
                    }
                    if !crate::ids::is_step_id(num) {
                        self.err(
                            p,
                            "structure.step-heading",
                            format!("`{num}` isn't a step number"),
                        );
                        return;
                    }
                    self.t
                        .steps
                        .push(new_step(num, num, stage, title.trim(), p.line));
                    self.cur = Some(Cur::Step(self.t.steps.len() - 1));
                } else if text.starts_with("Council setup") {
                    let title = text.split_once(':').map_or(text, |(_, t)| t.trim());
                    self.t.steps.push(new_step(
                        "council-setup",
                        "council-setup",
                        stage,
                        title,
                        p.line,
                    ));
                    self.cur = Some(Cur::Step(self.t.steps.len() - 1));
                    self.council = true;
                } else if let Some(rest) = text.strip_prefix("Stage ")
                    && rest.trim_end().ends_with("gate")
                {
                    if let Some(s) = self.t.stages.iter_mut().find(|s| s.number == stage) {
                        s.has_gate = true;
                    }
                    self.in_gate_heading = Some(stage);
                    self.gate_positions.push((stage, self.t.steps.len()));
                }
            }
            Section::Gate => {
                if let Some(rest) = text.strip_prefix("Gate prompt ") {
                    let (letter, title) = rest.split_once(':').unwrap_or((rest, ""));
                    let id = format!("gate.{}", letter.trim());
                    if crate::ids::is_step_id(&id) {
                        let mut s = new_step(&format!("0:{id}"), &id, 0, title.trim(), p.line);
                        s.template_id = Some(id);
                        self.t.gate_templates.push(s);
                        self.cur = Some(Cur::Gate(self.t.gate_templates.len() - 1));
                    } else {
                        self.err(
                            p,
                            "structure.step-heading",
                            format!("unknown gate prompt `{letter}`"),
                        );
                    }
                }
            }
            Section::AppendixF => {
                if text.starts_with("F.2 ") {
                    let mut s = new_step(
                        "0:F.2",
                        "F.2",
                        0,
                        text.split_once(' ').map_or(text, |(_, t)| t),
                        p.line,
                    );
                    s.template_id = Some("F.2".into());
                    self.t.f2_template = Some(s);
                    self.cur = Some(Cur::F2);
                }
            }
            Section::None => {}
        }
    }

    fn blockquote(&mut self, n: Node<'_>) {
        let p = pos(n);
        self.close_prompt(p);
        let Some(label) = quote_label(n) else { return };
        if label.starts_with("RUN WITH") {
            let text = source_lines(&self.lines, n, 1);
            if let Some(step) = self.cur_step()
                && step.run_with.is_none()
            {
                step.run_with = Some(RunWith { text, line: p.line });
            }
        } else if label.starts_with("INSPECTION") {
            self.inspection(n);
        } else if label.starts_with("HANDOFF")
            && let Section::Stage(stage) = self.section
        {
            let mut s = String::new();
            inline_text(n, &mut s);
            self.t.handoffs.push(HandoffNote {
                stage,
                text: collapse(&s),
                line: p.line,
            });
        }
    }

    fn inspection(&mut self, n: Node<'_>) {
        let Section::Stage(stage) = self.section else {
            return;
        };
        let Some(list) = n
            .children()
            .find(|c| matches!(c.data().value, NodeValue::List(_)))
        else {
            return;
        };
        for item in list.children() {
            let mut raw = String::new();
            inline_text(item, &mut raw);
            let normalized = collapse(&raw.to_lowercase());
            let text = item
                .first_child()
                .map(|para| source_lines(&self.lines, para, 1))
                .unwrap_or_default();
            let text = text
                .lines()
                .enumerate()
                .map(|(i, l)| {
                    if i == 0 {
                        l.trim_start()
                            .trim_start_matches(['-', '*', '+'])
                            .trim_start()
                    } else {
                        l.trim()
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
            self.t.items.push(InspectionItem {
                id: None,
                stage,
                text,
                text_sha256: item_hash(&normalized),
                normalized,
                kind: "attest".into(),
                line: pos(item).line,
            });
        }
    }

    fn code(&mut self, n: Node<'_>) {
        let p = pos(n);
        let NodeValue::CodeBlock(cb) = &n.data().value else {
            return;
        };
        let info = cb.info.trim().to_owned();
        let literal = cb.literal.clone();
        if literal.contains("<!-- fm:") {
            self.err(
                p,
                "grammar.not-block-level",
                "annotations must be block-level (this one is inside a fence)",
            );
        }
        if info == "markdown" && self.council {
            let name = literal
                .lines()
                .find_map(|l| l.strip_prefix("name:"))
                .map(|s| s.trim().to_owned())
                .unwrap_or_default();
            self.t.council_blocks.push(CouncilBlock {
                name,
                text: literal,
                line: p.line + 1,
            });
            self.prompt_open = false;
            return;
        }
        if info != "text" {
            self.close_prompt(p);
            return;
        }
        let cond = self
            .open_if
            .as_ref()
            .map(|(s, e, _)| (s.clone(), e.clone()));
        let open = self.prompt_open;
        let Some(step) = self.cur_step() else { return };
        // DR-008: the first text fence after the fm:step (or heading, for display when unannotated).
        if step.prompt.is_none() {
            step.prompt = Some(Prompt::default());
        } else if !open {
            return;
        }
        if let Some(pr) = step.prompt.as_mut() {
            pr.parts.push(PromptPart {
                text: literal,
                line: p.line + 1,
                cond,
            });
        }
        self.prompt_open = true;
    }

    fn html(&mut self, n: Node<'_>, prev: Option<Node<'_>>) {
        let p = pos(n);
        let NodeValue::HtmlBlock(hb) = &n.data().value else {
            return;
        };
        let literal = hb.literal.clone();
        let is_fm = literal.trim_start().starts_with("<!-- fm:");
        let first = !self.first_html_seen;
        self.first_html_seen = true;
        if !is_fm {
            self.close_prompt(p);
            if first {
                self.err(
                    p,
                    "grammar.missing-version",
                    "the first annotation after the title must be <!-- fm:grammar v=1 -->",
                );
            }
            return;
        }
        let d = match grammar::lex(&literal, p, self.file) {
            Ok(d) => d,
            Err(e) => {
                self.t.diagnostics.push(e);
                return;
            }
        };
        if first && d.name != "grammar" {
            self.err(
                p,
                "grammar.missing-version",
                "the first annotation after the title must be <!-- fm:grammar v=1 -->",
            );
        }
        match d.name.as_str() {
            "grammar" => self.grammar(&d, first),
            "if" | "end" => self.fm_if(&d),
            _ => {
                self.close_prompt(p);
                match d.name.as_str() {
                    "step" => self.fm_step(&d, n, prev),
                    "verify" => self.fm_verify(&d, n, prev),
                    other => {
                        let newer = self.t.grammar_version.unwrap_or(1) > grammar::GRAMMAR_VERSION;
                        if newer {
                            self.warn(
                                p,
                                "grammar.newer-grammar",
                                format!("unknown directive fm:{other} (newer grammar)"),
                            );
                        } else {
                            self.err(
                                p,
                                "grammar.unknown-directive",
                                format!("unknown directive fm:{other}"),
                            );
                        }
                    }
                }
            }
        }
    }

    fn grammar(&mut self, d: &Directive, first: bool) {
        if !first || self.grammar_seen {
            self.err(
                d.pos,
                "grammar.misplaced-version",
                "fm:grammar must be the first annotation, once",
            );
            return;
        }
        self.grammar_seen = true;
        match d
            .tokens
            .iter()
            .find(|t| t.key == "v")
            .and_then(|t| t.value.parse::<u32>().ok())
        {
            Some(v) if v >= 1 => self.t.grammar_version = Some(v),
            _ => self.err(
                d.pos,
                "grammar.missing-version",
                "fm:grammar needs v=<n> with n ≥ 1",
            ),
        }
    }

    fn fm_if(&mut self, d: &Directive) {
        if !self.prompt_open {
            self.err(
                d.pos,
                "grammar.misplaced-if",
                "fm:if / fm:end belong between the text fences of one prompt",
            );
            return;
        }
        if d.name == "if" {
            if self.open_if.is_some() {
                self.err(d.pos, "grammar.nested-if", "fm:if can't be nested");
                return;
            }
            let src = d
                .tokens
                .iter()
                .map(|t| {
                    if t.value.is_empty() {
                        t.key.clone()
                    } else {
                        format!("{}={}", t.key, t.value)
                    }
                })
                .collect::<String>();
            match super::parse_expr(&src) {
                Ok(e) => self.open_if = Some((src, e, d.pos.line)),
                Err(e) => {
                    let col = d.tokens.first().map_or(d.pos.column, |t| t.pos.column)
                        + u32::try_from(e.column()).unwrap_or(1)
                        - 1;
                    self.err(
                        SourcePos {
                            line: d.pos.line,
                            column: col,
                        },
                        e.rule(),
                        e.to_string(),
                    );
                }
            }
        } else if self.open_if.take().is_none() {
            self.err(d.pos, "grammar.unmatched-if", "fm:end without fm:if");
        }
    }

    fn is_run_with_just_before(n: Node<'_>, prev: Option<Node<'_>>) -> bool {
        prev.is_some_and(|q| {
            matches!(q.data().value, NodeValue::BlockQuote)
                && quote_label(q).is_some_and(|l| l.starts_with("RUN WITH"))
                && end_line(q) + 1 == n.data().sourcepos.start.line
        })
    }

    fn fm_step(&mut self, d: &Directive, n: Node<'_>, prev: Option<Node<'_>>) {
        let grammar = self.t.grammar_version.unwrap_or(1);
        let mut diags = Vec::new();
        let ann = grammar::parse_step(d, grammar, self.file, &mut diags);
        let stage = match self.section {
            Section::Stage(s) => Some(s),
            _ => None,
        };
        for x in &mut diags {
            x.stage = stage;
        }
        self.t.diagnostics.extend(diags);
        let Some(ann) = ann else { return };
        let after_run_with = Self::is_run_with_just_before(n, prev);
        let prev_is_prose = prev.is_some_and(|q| match q.data().value {
            NodeValue::Paragraph => {
                q.data().sourcepos.end.line + 1 == n.data().sourcepos.start.line
            }
            NodeValue::BlockQuote => {
                quote_label(q).is_some_and(|l| l.starts_with("HANDOFF"))
                    && end_line(q) + 1 == n.data().sourcepos.start.line
            }
            _ => false,
        });
        let heading_id = self
            .cur_step()
            .map(|s| (s.id.clone(), s.run_with.is_some(), s.annotation.is_some()));
        if after_run_with {
            let Some((hid, _, annotated)) = heading_id else {
                self.err(
                    d.pos,
                    "grammar.misplaced-step",
                    "fm:step after a RUN WITH that belongs to no step",
                );
                return;
            };
            if annotated {
                self.err(
                    d.pos,
                    "grammar.misplaced-step",
                    "a second fm:step under the same RUN WITH",
                );
                return;
            }
            if hid != ann.id {
                self.err(
                    d.pos,
                    "grammar.id-mismatch",
                    format!("fm:step id={} under the heading of {hid}", ann.id),
                );
                return;
            }
            self.bind(ann);
            return;
        }
        // Standalone (DR-007): after a prose paragraph or HANDOFF blockquote.
        if let Some((hid, has_rw, annotated)) = &heading_id
            && *hid == ann.id
        {
            if *has_rw || *annotated || !prev_is_prose {
                self.err(
                    d.pos,
                    "grammar.misplaced-step",
                    "fm:step must directly follow its RUN WITH (no blank line)",
                );
                return;
            }
            self.bind(ann);
            return;
        }
        if !prev_is_prose {
            self.err(d.pos, "grammar.misplaced-step", "a standalone fm:step must directly follow the paragraph or HANDOFF that describes it");
            return;
        }
        let Some(stage) = stage else {
            self.err(
                d.pos,
                "grammar.misplaced-step",
                "a standalone fm:step must be inside a stage",
            );
            return;
        };
        let title = prev
            .map(|q| {
                let mut s = String::new();
                inline_text(q, &mut s);
                let s = collapse(&s);
                s.chars().take(80).collect::<String>()
            })
            .unwrap_or_default();
        let mut s = new_step(&ann.id, &ann.id, stage, &title, d.pos.line);
        if StepKey::parse(&ann.id).is_err() {
            self.err(
                d.pos,
                "grammar.bad-id",
                format!("`{}` isn't a stage step ID", ann.id),
            );
            return;
        }
        s.key = StepKey::parse(&ann.id).unwrap_or_else(|_| StepKey::instance(stage, "F.2"));
        self.t.steps.push(s);
        self.cur = Some(Cur::Step(self.t.steps.len() - 1));
        self.bind(ann);
    }

    fn bind(&mut self, ann: StepAnnotation) {
        if let Some(step) = self.cur_step() {
            step.kind = ann.kind;
            step.tag = ann.tag;
            step.session = ann.session;
            step.surface = ann.surface;
            step.cwd = ann.cwd;
            step.start = ann.start;
            step.out.clone_from(&ann.out);
            step.appends.clone_from(&ann.appends);
            step.status = if ann.unknown_keys.is_empty() {
                StepStatus::Runnable
            } else {
                StepStatus::NeedsNewerForeman
            };
            // A prompt bound under the heading before the annotation doesn't count (DR-008).
            step.prompt = None;
            step.annotation = Some(ann);
        }
        self.prompt_open = false;
    }

    fn fm_verify(&mut self, d: &Directive, n: Node<'_>, prev: Option<Node<'_>>) {
        let ok = prev.is_some_and(|q| {
            matches!(q.data().value, NodeValue::BlockQuote)
                && quote_label(q).is_some_and(|l| l.starts_with("INSPECTION"))
                && end_line(q) + 1 == n.data().sourcepos.start.line
        });
        if !ok {
            self.err(
                d.pos,
                "grammar.misplaced-verify",
                "fm:verify must directly follow its INSPECTION blockquote",
            );
            return;
        }
        let Section::Stage(stage) = self.section else {
            return;
        };
        let grammar = self.t.grammar_version.unwrap_or(1);
        let mut diags = Vec::new();
        let entries: Vec<VerifyEntry> = grammar::parse_verify(d, grammar, self.file, &mut diags);
        for x in &mut diags {
            x.stage = Some(stage);
        }
        self.t.diagnostics.extend(diags);
        // The bullets of the INSPECTION just before: the last `count` items of this stage.
        let count = prev
            .and_then(|q| {
                q.children()
                    .find(|c| matches!(c.data().value, NodeValue::List(_)))
            })
            .map_or(0, |l| l.children().count());
        if entries.len() != count {
            self.err(
                d.pos,
                "grammar.verify-count",
                format!("{} fm:verify entries for {count} bullets", entries.len()),
            );
        }
        let start = self.t.items.len().saturating_sub(count);
        for (item, e) in self.t.items[start..].iter_mut().zip(entries) {
            item.id = Some(e.id);
            item.kind = if e.kind.starts_with("auto:") {
                e.kind
            } else {
                e.kind.clone()
            };
        }
    }
}

/// Checks container depth iteratively; returns the first node beyond the limit.
fn too_deep<'a>(root: Node<'a>) -> Option<SourcePos> {
    let mut stack: Vec<(Node<'a>, usize)> = vec![(root, 0)];
    while let Some((n, d)) = stack.pop() {
        let container = matches!(
            n.data().value,
            NodeValue::BlockQuote | NodeValue::List(_) | NodeValue::Item(_)
        );
        let d = if container { d + 1 } else { d };
        if d > MAX_DEPTH {
            return Some(pos(n));
        }
        for c in n.children() {
            stack.push((c, d));
        }
    }
    None
}

/// Finds `fm:` comments that aren't top-level HTML blocks (DR-020).
fn not_block_level<'a>(root: Node<'a>) -> Vec<SourcePos> {
    let mut out = Vec::new();
    let mut stack: Vec<Node<'a>> = root.children().collect();
    while let Some(n) = stack.pop() {
        let top = n
            .parent()
            .is_some_and(|p| matches!(p.data().value, NodeValue::Document));
        match &n.data().value {
            NodeValue::HtmlInline(h) if h.contains("<!-- fm:") => out.push(pos(n)),
            NodeValue::HtmlBlock(h) if !top && h.literal.contains("<!-- fm:") => out.push(pos(n)),
            _ => {}
        }
        stack.extend(n.children());
    }
    out.sort_by_key(|p| (p.line, p.column));
    out
}

fn packet_formats(t: &mut Template) {
    let mut formats = Vec::new();
    let owners = t
        .steps
        .iter()
        .map(|s| (Some(s.stage), s))
        .chain(t.gate_templates.iter().map(|s| (None, s)));
    for (stage, step) in owners {
        let Some(pr) = &step.prompt else { continue };
        for part in &pr.parts {
            let lines: Vec<&str> = part.text.lines().collect();
            let mut i = 0;
            while i < lines.len() {
                let l = lines[i];
                if l.starts_with("=== HANDOFF PACKET · Stage ") && l.ends_with("===") {
                    let header_line = part.line + u32::try_from(i).unwrap_or(0);
                    let mut keys: Vec<FormatKey> = Vec::new();
                    let mut j = i + 1;
                    while j < lines.len() && !lines[j].trim().is_empty() {
                        let lj = lines[j];
                        if lj.starts_with([' ', '\t']) {
                            if let Some(k) = keys.last_mut() {
                                k.placeholder.push('\n');
                                k.placeholder.push_str(lj.trim());
                            }
                        } else if let Some((k, v)) = lj.split_once(':') {
                            let key_ok = k.starts_with(|c: char| c.is_ascii_uppercase())
                                && k.len() <= 60
                                && k.chars()
                                    .all(|c| c.is_ascii_alphanumeric() || " ()+/&.-".contains(c));
                            if key_ok {
                                keys.push(FormatKey {
                                    key: k.to_owned(),
                                    placeholder: v.trim().to_owned(),
                                    line: part.line + u32::try_from(j).unwrap_or(0),
                                });
                            }
                        }
                        j += 1;
                    }
                    formats.push(PacketFormat {
                        stage: if step.template_id.as_deref() == Some("gate.B") {
                            None
                        } else {
                            stage
                        },
                        step: step.id.clone(),
                        header: l.to_owned(),
                        keys,
                        line: header_line,
                    });
                    i = j;
                } else {
                    i += 1;
                }
            }
        }
    }
    t.packet_formats = formats;
}

/// Parses the structure (no cross-step validation).
pub(super) fn parse(source: &str, file: &str) -> (Template, Extras) {
    let norm = normalize_source(source);
    let mut t = Template {
        schema_version: 1,
        parser_version: PARSER_VERSION,
        grammar_version: None,
        blueprint_version: String::new(),
        source_sha256: source_sha256(source),
        stages: Vec::new(),
        steps: Vec::new(),
        gate_templates: Vec::new(),
        f2_template: None,
        items: Vec::new(),
        packet_formats: Vec::new(),
        handoffs: Vec::new(),
        council_blocks: Vec::new(),
        diagnostics: Vec::new(),
    };
    let lines: Vec<&str> = norm.split('\n').collect();
    if lines.len() > SOFT_LINE_LIMIT {
        t.diagnostics.push(Diagnostic::warning(
            file,
            SOFT_LINE_LIMIT,
            1,
            "structure.soft-limit",
            format!("{} lines (soft limit {SOFT_LINE_LIMIT})", lines.len()),
        ));
    }
    for (i, l) in lines.iter().enumerate() {
        let lower = l.to_ascii_lowercase();
        if let Some(c) = lower.find("bypasspermissions") {
            t.diagnostics.push(Diagnostic::error(
                file,
                i + 1,
                c + 1,
                "grammar.bypass-permissions",
                "bypassPermissions is never allowed (SR-03)",
            ));
        }
        if t.blueprint_version.is_empty()
            && let Some(rest) = l.strip_prefix("Version ")
        {
            rest.split_whitespace()
                .next()
                .unwrap_or_default()
                .clone_into(&mut t.blueprint_version);
        }
    }
    // Pre-scan nesting: a line with more than 64 leading container markers is refused before
    // comrak builds a tree that deep (DR-015).
    for (i, l) in lines.iter().enumerate() {
        let depth = l
            .bytes()
            .take_while(|b| matches!(b, b'>' | b' ' | b'-' | b'*' | b'+'))
            .filter(|b| *b == b'>')
            .count();
        if depth > MAX_DEPTH {
            t.diagnostics.push(Diagnostic::error(
                file,
                i + 1,
                1,
                "structure.depth-limit",
                format!("nested deeper than {MAX_DEPTH} levels"),
            ));
            return (t, Extras::default());
        }
    }
    let arena = Arena::new();
    let mut opts = Options::default();
    opts.parse.sourcepos_chars = false;
    let root = comrak::parse_document(&arena, &norm, &opts);
    if let Some(p) = too_deep(root) {
        t.diagnostics.push(Diagnostic::error(
            file,
            p.line as usize,
            p.column as usize,
            "structure.depth-limit",
            format!("nested deeper than {MAX_DEPTH} levels"),
        ));
        return (t, Extras::default());
    }
    for p in not_block_level(root) {
        t.diagnostics.push(Diagnostic::error(
            file,
            p.line as usize,
            p.column as usize,
            "grammar.not-block-level",
            "annotations must be block-level",
        ));
    }
    let mut w = Walker {
        file,
        lines,
        t,
        section: Section::None,
        cur: None,
        prompt_open: false,
        open_if: None,
        grammar_seen: false,
        first_html_seen: false,
        gate_positions: Vec::new(),
        in_gate_heading: None,
        council: false,
        stage_numbers: BTreeSet::new(),
        marks: Vec::new(),
    };
    let mut prev: Option<Node<'_>> = None;
    for n in root.children() {
        match &n.data().value {
            NodeValue::Heading(h) => {
                let level = h.level;
                w.heading(n, level);
            }
            NodeValue::BlockQuote => w.blockquote(n),
            NodeValue::CodeBlock(_) => w.code(n),
            NodeValue::HtmlBlock(_) => w.html(n, prev),
            _ => w.close_prompt(pos(n)),
        }
        prev = Some(n);
    }
    w.close_prompt(SourcePos::default());
    if !w.grammar_seen
        && !w
            .t
            .diagnostics
            .iter()
            .any(|d| d.rule == "grammar.missing-version")
    {
        w.t.diagnostics.push(Diagnostic::error(
            file,
            1,
            1,
            "grammar.missing-version",
            "the blueprint has no <!-- fm:grammar v=1 --> line",
        ));
    }
    let mut extras = Extras {
        gate_positions: std::mem::take(&mut w.gate_positions),
        ..Extras::default()
    };
    let total = u32::try_from(w.lines.len() + 1).unwrap_or(u32::MAX);
    for (i, (sec, start)) in w.marks.iter().enumerate() {
        let end = w.marks.get(i + 1).map_or(total, |m| m.1);
        match sec {
            Section::Stage(n) => extras.stage_ranges.push((*n, *start, end)),
            Section::Gate | Section::AppendixF => extras.template_ranges.push((*start, end)),
            Section::None => {}
        }
    }
    let mut t = w.t;
    packet_formats(&mut t);
    (t, extras)
}
