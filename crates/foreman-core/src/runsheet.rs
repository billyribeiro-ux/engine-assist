//! The run sheet (F-027, NFR-14, AC-F027-01, -03): a Markdown escape hatch for one stage, so a
//! stage can be run by hand without Foreman. For each applicable step it holds the resolved
//! prompt, the exact argv Foreman would pass (or the chat chips), the run inputs as filled, and
//! then the stage's inspection items with their kinds. Unresolved tokens never stop the export:
//! each is marked in place (`⟦UNRESOLVED …⟧`) and listed under the step.
//!
//! Pure: the app passes everything it read; the session IDs and the run-settings path are shown
//! as placeholders because they are chosen at launch.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::PathBuf;

use uuid::Uuid;

use crate::chain::{EffortOnResume, effort_carry};
use crate::ids::{Start, Surface};
use crate::launch::argv::{LaunchInputs, SessionArgs, SessionName, build_argv};
use crate::policy;
use crate::resolver::{self, PacketText, ResolveCtx, Unresolved};
use crate::rules::ApplicablePath;
use crate::schema::overlay::AddedItem;
use crate::schema::project::ProjectFile;
use crate::template::{Session, Step, StepKind, Template, lint};

/// Everything one stage's sheet is built from.
#[derive(Debug, Clone, Copy)]
pub struct SheetInput<'a> {
    /// The project's effective template.
    pub template: &'a Template,
    /// The approved `project.json`.
    pub project: &'a ProjectFile,
    /// The applicable path.
    pub path: &'a ApplicablePath,
    /// Packets by stage.
    pub packets: &'a BTreeMap<u8, PacketText>,
    /// Run inputs as filled, per step key.
    pub inputs: &'a BTreeMap<String, BTreeMap<String, String>>,
    /// Remembered inputs.
    pub remembered: &'a BTreeMap<String, String>,
    /// Optional-part toggles per step key.
    pub optional: &'a BTreeMap<String, BTreeMap<String, bool>>,
    /// Items attested "not applicable".
    pub attested_na: &'a BTreeSet<String>,
    /// Overlay items.
    pub overlay_items: &'a [AddedItem],
    /// The stage.
    pub stage: u8,
    /// `YYYY-MM-DD`.
    pub generated_on: &'a str,
}

/// The marker around an unresolved token (AC-F027-03).
#[must_use]
pub fn mark(what: &str) -> String {
    format!("⟦UNRESOLVED {what}⟧")
}

/// Placeholders shown instead of the values chosen at launch.
const NEW_SESSION: &str = "<new session id>";
const CHAIN_SESSION: &str = "<chain session id>";
const RUN_SETTINGS: &str = "<app data>/runs/<run id>/settings.json";
const PROMPT: &str = "<the prompt above>";

fn v4(byte: u8) -> Uuid {
    uuid::Builder::from_random_bytes([byte; 16]).into_uuid()
}

/// A step's prompt, resolved; what couldn't be resolved is marked and listed (never refused).
fn resolve_marked(input: &SheetInput<'_>, step: &Step) -> (String, Vec<String>) {
    let key = step.key.as_str();
    let mut project = input.project.clone();
    let mut packets = input.packets.clone();
    let mut inputs = input.inputs.get(key).cloned().unwrap_or_default();
    let optional = input.optional.get(key).cloned().unwrap_or_default();
    let mut unresolved: Vec<String> = Vec::new();
    // Each round fills every reported gap with its marker; a gap that can't be filled ends it.
    for _ in 0..8 {
        let ctx = ResolveCtx {
            template: input.template,
            step,
            project: &project,
            path: input.path,
            packets: &packets,
            inputs: &inputs,
            remembered: input.remembered,
            optional: &optional,
            attested_na: input.attested_na,
            overlay_items: input.overlay_items,
        };
        let gaps = match resolver::resolve(&ctx) {
            Ok(p) => return (p.text, unresolved),
            Err(g) => g,
        };
        let mut stuck = false;
        for g in gaps {
            match g {
                Unresolved::Variable { token } => {
                    let name = token.trim_start_matches('[').trim_end_matches(']').to_owned();
                    unresolved.push(format!("{token}: the project variable is empty"));
                    project.variables.insert(name, mark(&token));
                }
                Unresolved::RunInputMissing { name } => {
                    unresolved.push(format!("run input {name}: not filled in"));
                    inputs.insert(name.clone(), mark(&format!("run input {name}")));
                }
                Unresolved::PacketUnsigned { stage } | Unresolved::PacketMissing { stage } => {
                    unresolved.push(format!("the Stage {stage} packet: not signed yet"));
                    packets.insert(
                        stage,
                        PacketText {
                            text: mark(&format!("Stage {stage} packet")),
                            signed: true,
                        },
                    );
                }
                Unresolved::Undecided { message } => {
                    unresolved.push(message);
                    stuck = true;
                }
                Unresolved::UnknownToken { token } => {
                    unresolved.push(format!("{token}: not a token Foreman knows"));
                    stuck = true;
                }
            }
        }
        if stuck {
            break;
        }
    }
    // Not resolvable: the template text with every bracket token marked.
    let raw = step
        .prompt
        .as_ref()
        .map(crate::template::Prompt::joined)
        .unwrap_or_default();
    let declared: Vec<String> = step.inputs().iter().map(|i| i.name.clone()).collect();
    let (tokens, _) = resolver::tokens::scan(&raw, &declared);
    let mut out = String::with_capacity(raw.len());
    let mut at = 0;
    for t in tokens
        .iter()
        .filter(|t| t.class != Some(resolver::tokens::TokenClass::OptionalPart))
        .filter(|t| t.class != Some(resolver::tokens::TokenClass::Literal))
    {
        if t.offset < at {
            continue;
        }
        out.push_str(&raw[at..t.offset]);
        out.push_str(&mark(&t.text));
        at = t.offset + t.len;
    }
    out.push_str(&raw[at..]);
    unresolved.dedup();
    (out, unresolved)
}

/// The argv Foreman would pass for a CLI step (DR-122), with the launch-time values as
/// placeholders. `None` when the step has no complete start.
fn argv_line(input: &SheetInput<'_>, step: &Step, start: Start) -> Option<Vec<String>> {
    let session = if step.session == Session::Continue {
        SessionArgs::Resume { id: v4(2) }
    } else {
        SessionArgs::New { id: v4(1) }
    };
    let carry = effort_carry(&session, start.effort, EffortOnResume::Flag);
    let inputs = LaunchInputs {
        session,
        start,
        pass_effort: carry.flag,
        name: SessionName::for_step(&input.project.slug, step.stage, step.key.as_str()),
        settings: PathBuf::from("/fm-run-settings.json"),
        screen_reader: false,
        prompt: Some("PROMPT".to_owned()),
    };
    let argv = build_argv(&inputs).ok()?;
    let new = v4(1).hyphenated().to_string();
    let chain = v4(2).hyphenated().to_string();
    let mut out = vec!["claude".to_owned()];
    let n = argv.as_slice().len();
    for (i, a) in argv.as_slice().iter().enumerate() {
        out.push(if i + 1 == n {
            PROMPT.to_owned()
        } else if *a == new {
            NEW_SESSION.to_owned()
        } else if *a == chain {
            CHAIN_SESSION.to_owned()
        } else if a == "/fm-run-settings.json" {
            RUN_SETTINGS.to_owned()
        } else {
            a.clone()
        });
    }
    Some(out)
}

/// The longest run of backticks in `s`, plus one (at least 3): a fence that can't close early.
fn fence(s: &str) -> String {
    let mut best = 0;
    let mut run = 0;
    for c in s.chars() {
        if c == '`' {
            run += 1;
            best = best.max(run);
        } else {
            run = 0;
        }
    }
    "`".repeat((best + 1).max(3))
}

fn where_of(step: &Step) -> &'static str {
    match step.cwd {
        Some(crate::ids::CwdKind::Spec) => "spec/",
        _ => "the project folder",
    }
}

/// One stage's run sheet (AC-F027-01, -03).
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "one linear Markdown document: header, steps, items"
)]
pub fn run_sheet(input: &SheetInput<'_>) -> String {
    let t = input.template;
    let n = input.stage;
    let title = t
        .stages
        .iter()
        .find(|s| s.number == n)
        .map_or("", |s| s.title.as_str());
    let mut md = String::new();
    let _ = writeln!(
        md,
        "# Run sheet: {} · Stage {n} · {title}\n",
        input.project.name
    );
    let _ = writeln!(
        md,
        "Exported by Foreman on {} from blueprint {}. Run the steps in order. Session IDs and the \
         run-settings path are chosen when Foreman launches; by hand, leave out `--settings` and \
         use `--session-id` with a new UUID for a chain start and `--resume <that id>` to continue.\n",
        input.generated_on, t.blueprint_version
    );
    let steps: Vec<&Step> = t
        .steps
        .iter()
        .filter(|s| s.stage == n)
        .filter(|s| {
            input
                .path
                .step(s.key.as_str())
                .is_some_and(crate::rules::Applicability::on_path)
        })
        .collect();
    if steps.is_empty() {
        md.push_str("No step of this stage applies to this project.\n\n");
    }
    for s in steps {
        let _ = writeln!(md, "## {} {}\n", s.id, s.title);
        let start = policy::policy_start(s, &input.project.model_policy)
            .ok()
            .map(|e| e.value);
        match (s.surface, s.kind) {
            (_, StepKind::Action | StepKind::Manual | StepKind::Wait) => {
                let _ = writeln!(md, "- Kind: {} (no prompt)", s.kind.as_str());
            }
            (Some(Surface::Chat), _) => {
                let mut chips = lint::chat_chips(t, s, &input.project.name);
                if let Some(st) = start {
                    chips.push(format!("{} effort", st.effort.as_str()));
                }
                let _ = writeln!(md, "- Surface: claude.ai chat");
                if !chips.is_empty() {
                    let _ = writeln!(md, "- Chat setup: {}", chips.join(" · "));
                }
            }
            (Some(Surface::Desktop), _) => {
                let _ = writeln!(
                    md,
                    "- Surface: Claude Desktop (`claude --desktop` in {}), then pick {}",
                    where_of(s),
                    start.map_or_else(String::new, |st| st.to_string())
                );
            }
            (Some(Surface::Cli), _) => {
                let _ = writeln!(md, "- Surface: Claude Code CLI in {}", where_of(s));
            }
            (None, _) => {}
        }
        for cue in policy::switch_cues(s, &input.project.model_policy) {
            let _ = writeln!(md, "- Switch: {cue}");
        }
        if !s.out.is_empty() {
            let _ = writeln!(md, "- Writes: {}", s.out.join(", "));
        }
        let declared = s.inputs();
        if declared.is_empty() {
            md.push_str("- Run inputs: none\n");
        } else {
            let filled = input.inputs.get(s.key.as_str());
            md.push_str("- Run inputs:\n");
            for d in declared {
                let v = filled.and_then(|m| m.get(&d.name)).map(String::as_str);
                match v {
                    Some(v) if !v.trim().is_empty() => {
                        let one = v.lines().next().unwrap_or_default();
                        let more = if v.lines().count() > 1 { " …" } else { "" };
                        let _ = writeln!(md, "  - {} = {one}{more}", d.name);
                    }
                    _ => {
                        let _ = writeln!(md, "  - {} = {}", d.name, mark("(not filled in)"));
                    }
                }
            }
        }
        if s.prompt.is_none() || matches!(s.kind, StepKind::Action | StepKind::Wait) {
            md.push('\n');
            continue;
        }
        let (text, unresolved) = resolve_marked(input, s);
        if !unresolved.is_empty() {
            md.push_str("- Unresolved (marked ⟦UNRESOLVED …⟧ below):\n");
            for u in &unresolved {
                let _ = writeln!(md, "  - {u}");
            }
        }
        md.push('\n');
        let f = fence(&text);
        let _ = writeln!(md, "Prompt:\n\n{f}text\n{text}\n{f}\n");
        if s.surface == Some(Surface::Cli)
            && let Some(st) = start
            && let Some(argv) = argv_line(input, s, st)
        {
            md.push_str("Argv (one argument per line):\n\n```text\n");
            for a in argv {
                let _ = writeln!(md, "{a}");
            }
            md.push_str("```\n\n");
        }
    }
    let items: Vec<(String, String)> = t
        .items
        .iter()
        .filter(|i| i.stage == n)
        .map(|i| (i.text.clone(), i.kind.clone()))
        .chain(
            input
                .overlay_items
                .iter()
                .filter(|i| i.stage == n)
                .map(|i| (i.text.clone(), i.kind.clone())),
        )
        .collect();
    let _ = writeln!(md, "## Inspection items\n");
    if items.is_empty() {
        md.push_str("This stage has no inspection items.\n");
    }
    for (text, kind) in items {
        let _ = writeln!(md, "- [ ] {text} ({kind})");
    }
    md
}
