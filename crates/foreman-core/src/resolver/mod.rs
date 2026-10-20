//! Prompt resolution (F-006, DR-080–DR-093): a pure function of the template, the approved project,
//! the overlay, the signed packets, the run inputs, the optional-part toggles and the stage context.
//! Text inside a packet or an input is inserted as text and never changes a flag, mode, model, cwd
//! or destination (DR-093).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

pub mod tokens;

use tokens::{PACKET_MARKERS, Token, TokenClass};

use crate::ids::{CwdKind, TriState};
use crate::rules::{self, ApplicablePath, StageState};
use crate::schema::overlay::AddedItem;
use crate::schema::project::ProjectFile;
use crate::template::{Step, Template};

/// Values over this many bytes are accepted with a warning (DR-089).
pub const LARGE_INPUT: usize = 64 * 1024;

/// A signed (or not yet signed) packet's text: header line through the last field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PacketText {
    /// The text.
    pub text: String,
    /// Signed (by Foreman's signing, or confirmed by Billy, DR-135).
    pub signed: bool,
}

/// Everything resolution depends on (DR-093).
#[derive(Debug, Clone, Copy)]
pub struct ResolveCtx<'a> {
    /// The template.
    pub template: &'a Template,
    /// The step.
    pub step: &'a Step,
    /// The approved `project.json`.
    pub project: &'a ProjectFile,
    /// The applicable path.
    pub path: &'a ApplicablePath,
    /// Packets by stage.
    pub packets: &'a BTreeMap<u8, PacketText>,
    /// Run inputs typed for this step.
    pub inputs: &'a BTreeMap<String, String>,
    /// Remembered inputs (prefill for `:remember`).
    pub remembered: &'a BTreeMap<String, String>,
    /// Optional-part toggles (`opt-1` → included; default on).
    pub optional: &'a BTreeMap<String, bool>,
    /// Inspection items attested "not applicable".
    pub attested_na: &'a BTreeSet<String>,
    /// Overlay items.
    pub overlay_items: &'a [AddedItem],
}

/// One resolver decision (`docs/contracts/ipc.md` `PromptSegment`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum PromptSegment {
    /// Literal prompt text.
    Text {
        /// Text.
        text: String,
    },
    /// A token replaced.
    Resolved {
        /// The token.
        token: String,
        /// Its class.
        class: TokenClass,
        /// The inserted text.
        text: String,
        /// A remap note.
        note: Option<String>,
    },
    /// An optional part.
    OptionalPart {
        /// `opt-N`.
        part_id: String,
        /// Include toggle.
        included: bool,
    },
    /// An `fm:if` part removed.
    RemovedPart {
        /// The expression.
        expr: String,
        /// Why.
        reason: String,
    },
}

/// Why a prompt can't be resolved yet (each maps to a launch blocker).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Unresolved {
    /// A project variable is empty or still a placeholder (DR-083).
    Variable {
        /// Token.
        token: String,
    },
    /// "Sign Stage N first" (DR-084).
    PacketUnsigned {
        /// Stage.
        stage: u8,
    },
    /// The packet file is missing (`SignedPacketMissing`).
    PacketMissing {
        /// Stage.
        stage: u8,
    },
    /// A run input is required (DR-089).
    RunInputMissing {
        /// Input name.
        name: String,
    },
    /// An `fm:if` part depends on an undecided condition (DR-091).
    Undecided {
        /// "Decide <condition> to continue".
        message: String,
    },
    /// A token the classifier doesn't know (a template error).
    UnknownToken {
        /// Token.
        token: String,
    },
}

/// A resolved prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedPrompt {
    /// The text sent.
    pub text: String,
    /// SHA-256 of the text.
    pub sha256: String,
    /// Segments, in order.
    pub segments: Vec<PromptSegment>,
    /// Remap and size notes shown on the step.
    pub notes: Vec<String>,
    /// Lines for the next packet's `SKIPPED:` (removed parts, DR-090, DR-091).
    pub skipped: Vec<String>,
}

/// DR-092: a repo-relative path rendered relative to the step's cwd.
#[must_use]
pub fn render_for_cwd(repo_path: &str, cwd: Option<CwdKind>) -> String {
    match cwd {
        Some(CwdKind::Spec) => repo_path
            .strip_prefix("spec/")
            .map_or_else(|| format!("../{repo_path}"), str::to_owned),
        _ => repo_path.to_owned(),
    }
}

fn is_placeholder(token: &str, v: &str) -> bool {
    let t = v.trim();
    t.is_empty() || t == token || (t.starts_with('[') && t.ends_with(']'))
}

/// The latest stage before `n` (inclusive of `n` itself when `inclusive`) that applies, used for
/// remaps (DR-087, DR-088).
fn latest_applicable_at_or_before(path: &ApplicablePath, n: u8) -> Option<u8> {
    if path.stage_applies(n) {
        Some(n)
    } else {
        path.previous_applicable_stage(n)
    }
}

struct Out<'a> {
    ctx: &'a ResolveCtx<'a>,
    text: String,
    segments: Vec<PromptSegment>,
    notes: Vec<String>,
    skipped: Vec<String>,
    missing: Vec<Unresolved>,
}

impl Out<'_> {
    fn lit(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        self.text.push_str(s);
        if let Some(PromptSegment::Text { text }) = self.segments.last_mut() {
            text.push_str(s);
        } else {
            self.segments
                .push(PromptSegment::Text { text: s.to_owned() });
        }
    }

    fn resolved(&mut self, token: &str, class: TokenClass, text: String, note: Option<String>) {
        self.text.push_str(&text);
        if let Some(n) = &note {
            self.notes.push(n.clone());
        }
        self.segments.push(PromptSegment::Resolved {
            token: token.to_owned(),
            class,
            text,
            note,
        });
    }

    fn packet(&mut self, n: u8) -> Option<String> {
        match self.ctx.packets.get(&n) {
            None => {
                self.missing.push(Unresolved::PacketMissing { stage: n });
                None
            }
            Some(p) if !p.signed => {
                self.missing.push(Unresolved::PacketUnsigned { stage: n });
                None
            }
            Some(p) => Some(p.text.trim_end_matches('\n').to_owned()),
        }
    }

    fn marker(&mut self, t: &Token) {
        let Some((_, stages)) = PACKET_MARKERS.iter().find(|(m, _)| *m == t.inner) else {
            return;
        };
        if stages.is_empty() {
            // The inspection list of the instantiating stage (DR-085).
            let stage = self.ctx.step.stage;
            let lines: Vec<String> = self
                .ctx
                .template
                .items
                .iter()
                .filter(|i| i.stage == stage)
                .filter(|i| {
                    !i.id
                        .as_ref()
                        .is_some_and(|id| self.ctx.attested_na.contains(id))
                })
                .map(|i| format!("- {}", i.text))
                .chain(
                    self.ctx
                        .overlay_items
                        .iter()
                        .filter(|i| i.stage == stage && !self.ctx.attested_na.contains(&i.id))
                        .map(|i| format!("- {}", i.text)),
                )
                .collect();
            self.resolved(&t.text, TokenClass::PacketMarker, lines.join("\n"), None);
            return;
        }
        let mut used: Vec<u8> = Vec::new();
        let mut notes = Vec::new();
        for &n in *stages {
            let Some(m) = latest_applicable_at_or_before(self.ctx.path, n) else {
                notes.push(format!(
                    "Stage {n} was not applicable and no earlier packet exists"
                ));
                continue;
            };
            if m != n {
                notes.push(format!(
                    "Stage {n} was not applicable; using Stage {m}'s packet"
                ));
            }
            if used.contains(&m) {
                notes.push(format!("Stage {m}'s packet is included once"));
                continue;
            }
            used.push(m);
        }
        let mut parts = Vec::new();
        for n in &used {
            if let Some(p) = self.packet(*n) {
                parts.push(p);
            }
        }
        let note = (!notes.is_empty()).then(|| notes.join("; "));
        self.resolved(&t.text, TokenClass::PacketMarker, parts.join("\n\n"), note);
    }
}

/// The run inputs a step needs after part removal (DR-089: only tokens that survive).
#[must_use]
pub fn required_inputs(ctx: &ResolveCtx<'_>) -> Vec<String> {
    let (text, _, _, _) = kept_text(ctx);
    let declared: Vec<String> = ctx.step.inputs().iter().map(|i| i.name.clone()).collect();
    let (toks, _) = tokens::scan(&text, &declared);
    let excluded: BTreeSet<String> = toks
        .iter()
        .filter(|t| t.class == Some(TokenClass::OptionalPart))
        .filter(|t| {
            !t.part
                .as_ref()
                .is_some_and(|p| ctx.optional.get(p).copied().unwrap_or(true))
        })
        .filter_map(|t| t.part.clone())
        .collect();
    let mut out = Vec::new();
    for t in &toks {
        if t.class == Some(TokenClass::RunInput)
            && t.inner != "N"
            && !t.part.as_ref().is_some_and(|p| excluded.contains(p))
        {
            let n = tokens::input_name(&t.inner);
            if !out.contains(&n) {
                out.push(n);
            }
        }
    }
    out
}

/// The prompt text after `fm:if` parts are kept or removed (DR-091).
fn kept_text(ctx: &ResolveCtx<'_>) -> (String, Vec<PromptSegment>, Vec<String>, Vec<Unresolved>) {
    let mut text = String::new();
    let mut segs = Vec::new();
    let mut skipped = Vec::new();
    let mut missing = Vec::new();
    let Some(prompt) = &ctx.step.prompt else {
        return (text, segs, skipped, missing);
    };
    for part in &prompt.parts {
        match &part.cond {
            None => text.push_str(&part.text),
            Some((src, e)) => match rules::eval(e, &ctx.project.conditions) {
                TriState::Yes => text.push_str(&part.text),
                TriState::No => {
                    let all_no = e
                        .conditions()
                        .iter()
                        .all(|c| ctx.project.conditions.get(c) == Some(&TriState::No));
                    let reason = if all_no {
                        "all false".to_owned()
                    } else {
                        rules::explain(e, &ctx.project.conditions)
                    };
                    skipped.push(format!(
                        "{} part \"fm:if {src}\": not applicable ({reason})",
                        ctx.step.key
                    ));
                    segs.push(PromptSegment::RemovedPart {
                        expr: src.clone(),
                        reason,
                    });
                }
                TriState::Undecided => {
                    let names: Vec<&str> = e
                        .conditions()
                        .into_iter()
                        .filter(|c| {
                            ctx.project
                                .conditions
                                .get(c)
                                .copied()
                                .unwrap_or(TriState::Undecided)
                                == TriState::Undecided
                        })
                        .map(crate::ids::ConditionId::as_str)
                        .collect();
                    missing.push(Unresolved::Undecided {
                        message: format!("Decide {} to continue", names.join(", ")),
                    });
                }
            },
        }
    }
    (text, segs, skipped, missing)
}

/// Resolves a step's prompt (DR-080–DR-093).
///
/// # Errors
/// Every unresolved token or blocking condition, in order.
#[allow(
    clippy::too_many_lines,
    reason = "one arm per token class (DR-082), kept together"
)]
pub fn resolve(ctx: &ResolveCtx<'_>) -> Result<ResolvedPrompt, Vec<Unresolved>> {
    let (text, removed, skipped, missing) = kept_text(ctx);
    let mut o = Out {
        ctx,
        text: String::new(),
        segments: removed,
        notes: Vec::new(),
        skipped,
        missing,
    };
    let declared: Vec<String> = ctx.step.inputs().iter().map(|i| i.name.clone()).collect();
    let (toks, _) = tokens::scan(&text, &declared);
    let stage = ctx.step.stage;
    // Optional parts: which are excluded, and the byte ranges to drop.
    let mut drop: Vec<(usize, usize)> = Vec::new();
    let mut excluded: BTreeSet<String> = BTreeSet::new();
    for t in toks
        .iter()
        .filter(|t| t.class == Some(TokenClass::OptionalPart))
    {
        let id = t.part.clone().unwrap_or_default();
        let on = ctx.optional.get(&id).copied().unwrap_or(true);
        o.segments.push(PromptSegment::OptionalPart {
            part_id: id.clone(),
            included: on,
        });
        if on {
            let prefix = if text[t.offset..].starts_with("[optional: ") {
                11
            } else {
                10
            };
            drop.push((t.offset, t.offset + prefix));
            drop.push((t.offset + t.len - 1, t.offset + t.len));
        } else {
            drop.push((t.offset, t.offset + t.len));
            excluded.insert(id.clone());
            o.skipped.push(format!(
                "{} part \"{id}\": skipped: Include turned off",
                ctx.step.key
            ));
        }
    }
    let mut pos = 0usize;
    let mut events: Vec<(usize, usize, Option<&Token>)> =
        drop.iter().map(|(a, b)| (*a, *b, None)).collect();
    for t in toks
        .iter()
        .filter(|t| t.class != Some(TokenClass::OptionalPart))
    {
        if t.part.as_ref().is_some_and(|p| excluded.contains(p)) {
            continue;
        }
        events.push((t.offset, t.offset + t.len, Some(t)));
    }
    events.sort_by_key(|(a, b, _)| (*a, *b));
    for (start, end, tok) in events {
        if start < pos {
            continue; // inside an excluded part
        }
        o.lit(&text[pos..start]);
        pos = end;
        let Some(t) = tok else { continue };
        match t.class {
            None => o.missing.push(Unresolved::UnknownToken {
                token: t.text.clone(),
            }),
            Some(TokenClass::Literal) => o.lit(&t.text),
            Some(TokenClass::ProjectVariable) => match ctx.project.variables.get(&t.inner) {
                Some(v) if !is_placeholder(&t.text, v) => {
                    o.resolved(&t.text, TokenClass::ProjectVariable, v.clone(), None)
                }
                _ => o.missing.push(Unresolved::Variable {
                    token: t.text.clone(),
                }),
            },
            Some(TokenClass::PacketMarker) => o.marker(t),
            Some(TokenClass::PathAlias) => match t.inner.as_str() {
                "app" => {
                    let after = &text[end..];
                    if let Some(rest_len) = after.strip_prefix("-spec/").map(|_| 6) {
                        pos = end + rest_len;
                        let rendered = render_for_cwd("spec/", ctx.step.cwd);
                        o.resolved(
                            "[app]-spec/",
                            TokenClass::PathAlias,
                            rendered,
                            Some("`[app]-spec/` → `spec/`".into()),
                        );
                    } else {
                        o.resolved(
                            &t.text,
                            TokenClass::PathAlias,
                            ctx.project.spec_folder.clone(),
                            Some("`[app]` → `spec`".into()),
                        );
                    }
                }
                "N-1" => {
                    let p = (0..stage)
                        .rev()
                        .find(|s| {
                            ctx.path.stage_applies(*s)
                                && ctx.packets.get(s).is_some_and(|x| x.signed)
                        })
                        .or_else(|| ctx.path.previous_applicable_stage(stage));
                    match p {
                        Some(p) => {
                            let note = (stage.checked_sub(1) != Some(p)).then(|| {
                                format!(
                                    "Stage {} has no packet; using Stage {p}'s",
                                    stage.saturating_sub(1)
                                )
                            });
                            o.resolved(&t.text, TokenClass::PathAlias, format!("{p:02}"), note);
                        }
                        None => o.missing.push(Unresolved::PacketMissing {
                            stage: stage.saturating_sub(1),
                        }),
                    }
                }
                "N+1" => match ctx.path.next_applicable_stage(stage) {
                    Some(n) => {
                        let padded = text[..start].ends_with("stage-");
                        let note = (n != stage + 1).then(|| {
                            format!("Stage {} doesn't apply; next is Stage {n}", stage + 1)
                        });
                        o.resolved(
                            &t.text,
                            TokenClass::PathAlias,
                            if padded {
                                format!("{n:02}")
                            } else {
                                n.to_string()
                            },
                            note,
                        );
                    }
                    None => o.resolved(
                        &t.text,
                        TokenClass::PathAlias,
                        (stage + 1).to_string(),
                        None,
                    ),
                },
                _ => o.missing.push(Unresolved::UnknownToken {
                    token: t.text.clone(),
                }),
            },
            Some(TokenClass::RunInput) => {
                if t.inner == "N" {
                    let padded = text[..start].ends_with("stage-");
                    o.resolved(
                        &t.text,
                        TokenClass::RunInput,
                        if padded {
                            format!("{stage:02}")
                        } else {
                            stage.to_string()
                        },
                        None,
                    );
                    continue;
                }
                let name = tokens::input_name(&t.inner);
                let remember = ctx
                    .step
                    .inputs()
                    .iter()
                    .any(|i| i.name == name && i.remember);
                let v = ctx
                    .inputs
                    .get(&name)
                    .or_else(|| remember.then(|| ctx.remembered.get(&name)).flatten());
                match v {
                    Some(v) if !v.trim().is_empty() => {
                        let note = (v.len() > LARGE_INPUT).then(|| format!("{name} is over 64 KB"));
                        o.resolved(&t.text, TokenClass::RunInput, v.clone(), note);
                    }
                    _ => o.missing.push(Unresolved::RunInputMissing { name }),
                }
            }
            Some(TokenClass::OptionalPart) => {}
        }
    }
    o.lit(&text[pos..]);
    if !o.missing.is_empty() {
        return Err(o.missing);
    }
    let sha256 = crate::store::sha256_hex(o.text.as_bytes());
    Ok(ResolvedPrompt {
        text: o.text,
        sha256,
        segments: o.segments,
        notes: o.notes,
        skipped: o.skipped,
    })
}

/// Whether a stage is on the applicable path (re-exported for callers building contexts).
#[must_use]
pub fn stage_applies(path: &ApplicablePath, n: u8) -> bool {
    !matches!(path.stage(n), Some(StageState::NotApplicable(_)))
}
