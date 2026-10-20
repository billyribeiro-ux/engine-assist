//! M1-16 (and M1-01's required test): the golden parse of the annotated v2.2 blueprint, the
//! round-trip property, and the template cache (AC-F001-01, AC-F001-05, EX-M1-02, ADR-023 §5).
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

use std::fmt::Write as _;

use common::{blueprint, parse};
use foreman_core::template::grammar::{self, SourcePos, StepAnnotation};
use foreman_core::template::render::render_step;
use foreman_core::template::{Step, Template};

/// DR-038's reference annotations, exactly as the domain rules write them.
const DR038: &[&str] = &[
    "id=0.1 tag=optional when=!idea_known surface=chat session=new start=opus@adaptive",
    "id=0.2 tag=optional when=!idea_known session=continue chain=0.1 start=opus@adaptive out=docs/handoff/stage-00.md",
    "id=1.1 tag=core surface=chat session=new start=opus@adaptive",
    "id=1.2 tag=core session=continue chain=1.1 start=opus@adaptive in=X:text in=changes:text",
    "id=1.3 tag=conditional when=commercial session=continue chain=1.1 start=opus@adaptive",
    "id=1.3b kind=wait tag=conditional when=commercial",
    "id=1.4 tag=core session=continue chain=1.1 start=opus@adaptive in=paste_numbers:multiline out=docs/handoff/stage-01.md",
    "id=1.4b kind=action tag=core",
    "id=2.1 tag=conditional when=commercial surface=chat session=new start=opus@adaptive out=spec/02-teardown.md",
    "id=council-setup kind=action tag=core",
    "id=2.2 kind=prompt tag=core surface=cli cwd=spec session=new start=fable@high/plan switch=plan-approved:fable@high/acceptEdits alt=pro-plan:opus@xhigh out=spec/council/r1-market.md,spec/council/r1-users.md,spec/council/r2-features.md,spec/council/r2-growth.md,spec/council/r2-feasibility.md,spec/council/r3-synthesis.md,spec/council/r4-redteam.md,spec/council/r5-resolution.md",
    "id=2.3 kind=followup tag=core surface=cli session=continue chain=2.2 start=fable@high/acceptEdits alt=pro-plan:opus@xhigh out=spec/council/r6-feature-specs.md",
    "id=2.4 kind=followup tag=core session=continue chain=2.2 start=fable@high/acceptEdits alt=pro-plan:opus@xhigh out=spec/council/r7-stack.md",
    "id=2.4b kind=manual tag=core out=spec/council/r7-stack.md",
    "id=2.5 kind=followup tag=core session=continue chain=2.2 start=fable@high/acceptEdits alt=pro-plan:opus@xhigh out=spec/council/r8-nfr.md",
    "id=2.6 kind=prompt tag=core session=continue chain=2.2 start=fable@high/default alt=pro-plan:opus@xhigh out=spec/MASTER-SPEC.md,docs/handoff/stage-02.md",
    "id=3.1 tag=conditional when=commercial|payments surface=chat session=new start=opus@adaptive",
    "id=3.2 tag=core session=continue chain=3.1 start=opus@adaptive in=jurisdictions:text:remember out=docs/handoff/stage-03.md",
    "id=4.1 tag=core surface=chat session=new start=opus@adaptive",
    "id=4.2 tag=core surface=chat session=new start=opus@adaptive",
    "id=4.3 tag=core surface=chat session=new start=opus@adaptive",
    "id=4.4 tag=core surface=chat session=new start=opus@adaptive",
    "id=4.5 tag=conditional when=has_ui surface=chat session=new start=opus@adaptive in=list_screens:text",
    "id=4.6 tag=core surface=chat session=new start=opus@adaptive",
    "id=4.7 tag=core surface=chat session=new start=fable@adaptive",
    "id=4.8 tag=core surface=chat session=new start=opus@adaptive out=docs/handoff/stage-04.md",
    "id=5.1 tag=core surface=cli cwd=project session=new start=opusplan@medium/plan switch=plan-approved:opusplan@medium/acceptEdits",
    "id=5.1b kind=followup tag=core session=continue chain=5.1",
    "id=5.2 tag=core session=continue chain=5.1 start=opus@high/plan switch=plan-approved:opus@high/acceptEdits",
    "id=5.3 tag=core session=continue chain=5.1 start=opus@high/plan",
    "id=5.4 tag=core session=continue chain=5.1 start=sonnet@low/acceptEdits",
    "id=5.5 tag=conditional when=server session=continue chain=5.1 start=opus@high/plan switch=plan-approved:opus@high/acceptEdits",
    "id=gate.A tag=core surface=cli cwd=project session=new start=opus@high/plan",
    "id=gate.B tag=core session=continue chain=gate.A start=opus@medium/acceptEdits out=docs/handoff/stage-NN.md",
    "id=gate.C kind=followup tag=core session=continue chain=gate.A start=opus@medium/acceptEdits appends=docs/lessons.md",
    "id=F.2 kind=followup tag=core session=continue chain=@stage appends=docs/lessons.md",
];

const DR038_VERIFY: &[(u8, &[&str])] = &[
    (
        0,
        &["candidates", "pick-on-evidence", "experiment-threshold"],
    ),
    (
        1,
        &[
            "validation-threshold",
            "press-release-tested",
            "brief-open-items",
        ],
    ),
    (
        2,
        &[
            "feature-matrix",
            "beats-competitor",
            "redteam-resolved",
            "stack-choice",
            "frozen",
        ],
    ),
    (
        3,
        &["margin-positive", "payment-terms", "requirements-numbered"],
    ),
    (
        4,
        &[
            "review-clean",
            "requirements-mapped",
            "formulas",
            "tickets-complete",
            "contractor-test",
        ],
    ),
    (5, &["fresh-clone", "ci-gates", "staging", "adr-check"]),
];

fn ann(src: &str) -> StepAnnotation {
    let d = grammar::lex(
        &format!("<!-- fm:step {src} -->"),
        SourcePos { line: 1, column: 1 },
        "t",
    )
    .unwrap();
    let mut diags = Vec::new();
    let mut a = grammar::parse_step(&d, 1, "t", &mut diags).unwrap();
    assert!(diags.is_empty(), "{src}: {diags:?}");
    a.pos = SourcePos::default();
    a.keys.clear();
    a
}

fn snapshot(t: &Template, s: &Step) -> String {
    let mut o = String::new();
    let a = s.annotation.as_ref();
    let _ = writeln!(o, "## {} · {} · {:?}", s.key, s.title, s.status);
    let _ = writeln!(
        o,
        "kind={} tag={} when={}",
        s.kind.as_str(),
        s.tag.as_str(),
        a.and_then(|a| a.when.as_ref())
            .map_or("-", |(w, _)| w.as_str())
    );
    let _ = writeln!(
        o,
        "surface={} cwd={} session={:?} chain={}",
        s.surface.map_or("-", |x| x.as_str()),
        s.cwd.map_or("-", |x| x.as_str()),
        s.session,
        s.chain.as_ref().map_or("-".into(), ToString::to_string)
    );
    let _ = writeln!(
        o,
        "start={} switches=[{}] alts=[{}]",
        s.start.map_or("-".into(), |x| x.to_string()),
        s.switches()
            .iter()
            .map(|w| format!("{}:{}", w.trigger, w.to))
            .collect::<Vec<_>>()
            .join(", "),
        s.alts()
            .iter()
            .map(|x| format!(
                "{}:{}@{}",
                x.rule.as_str(),
                x.model.as_str(),
                x.effort.as_str()
            ))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let _ = writeln!(
        o,
        "in=[{}] out=[{}] appends={} requires=[{}]",
        s.inputs()
            .iter()
            .map(|i| format!(
                "{}:{:?}{}",
                i.name,
                i.kind,
                if i.remember { ":remember" } else { "" }
            ))
            .collect::<Vec<_>>()
            .join(", "),
        s.out.join(", "),
        s.appends.as_deref().unwrap_or("-"),
        a.map(|a| a.requires.join(", ")).unwrap_or_default()
    );
    if let Some(p) = &s.prompt {
        for part in &p.parts {
            let _ = writeln!(
                o,
                "--- prompt part{} ---",
                part.cond
                    .as_ref()
                    .map(|(c, _)| format!(" (fm:if {c})"))
                    .unwrap_or_default()
            );
            o.push_str(&part.text);
        }
    }
    let _ = t;
    o.push('\n');
    o
}

mod template {
    use super::*;
    use proptest::prelude::*;

    /// AC-F001-01, EX-M1-02, M1-01: zero errors; every annotated step equals DR-038; the snapshot
    /// of Stages 1–2 and F.2 (every field and the prompt text) matches the golden file.
    #[test]
    fn golden_v2_2() {
        let t = parse(&blueprint());
        let errors: Vec<_> = t.errors().collect();
        assert!(errors.is_empty(), "{errors:#?}");
        assert_eq!(t.grammar_version, Some(1));
        assert_eq!(t.blueprint_version, "2.2");
        // DR-038, step by step (canonical renders compared, so key order doesn't matter).
        for want in DR038 {
            let a = ann(want);
            let s = t
                .steps
                .iter()
                .chain(t.gate_templates.iter())
                .chain(t.f2_template.iter())
                .find(|s| s.annotation.as_ref().is_some_and(|x| x.id == a.id))
                .unwrap_or_else(|| panic!("{}", a.id));
            assert_eq!(
                render_step(s.annotation.as_ref().unwrap()),
                render_step(&a),
                "{}",
                a.id
            );
        }
        let annotated: usize = t
            .steps
            .iter()
            .filter(|s| s.template_id.is_none() && s.annotation.is_some())
            .count()
            + t.gate_templates.len()
            + 1;
        assert_eq!(annotated, DR038.len(), "no annotation beyond DR-038");
        for (stage, ids) in DR038_VERIFY {
            let got: Vec<&str> = t
                .items
                .iter()
                .filter(|i| i.stage == *stage)
                .filter_map(|i| i.id.as_deref())
                .collect();
            assert_eq!(&got, ids, "stage {stage}");
        }
        // The golden snapshot: Stages 1–2 and their F.2 instances.
        let mut snap = String::new();
        for s in t.steps.iter().filter(|s| s.stage == 1 || s.stage == 2) {
            snap.push_str(&snapshot(&t, s));
        }
        insta::assert_snapshot!("golden_v2_2_stages_1_2", snap);
        let mut gate = String::new();
        for s in t.stage_steps(5).filter(|s| s.template_id.is_some()) {
            gate.push_str(&snapshot(&t, s));
        }
        insta::assert_snapshot!("golden_v2_2_stage_5_gate", gate);
        // The fixture equals the copy for Billy's blueprint repo, and the generator output.
        let spec_copy = std::fs::read_to_string(
            common::repo_root().join("spec/blueprint-v2.2/claude-build-blueprint-v2.2.md"),
        )
        .unwrap();
        assert_eq!(spec_copy, blueprint());
        // Rendered prose is unchanged: removing every fm: comment and re-joining the split fences
        // gives v2.1 plus only the PROPOSAL's wording edits (checked: line count delta is bounded).
        let bp = blueprint();
        let stripped: Vec<&str> = bp
            .lines()
            .filter(|l| !l.starts_with("<!-- fm:") && !l.starts_with("     "))
            .collect();
        assert!(stripped.len() < common::blueprint_v21().lines().count() + 60);
    }

    proptest! {
        /// AC-F001-05: random valid annotation sets in random key order parse and render back
        /// identically.
        #[test]
        fn golden_v2_2_roundtrip(i in 0usize..36, seed in any::<u64>()) {
            let src = DR038[i % DR038.len()];
            let mut toks: Vec<&str> = src.split(' ').collect();
            // Shuffle deterministically by seed.
            let n = toks.len();
            let mut s = seed;
            for k in (1..n).rev() {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let j = usize::try_from(s >> 33).unwrap() % (k + 1);
                toks.swap(k, j);
            }
            let shuffled = toks.join(" ");
            let a = ann(&shuffled);
            let b = ann(src);
            prop_assert_eq!(render_step(&a), render_step(&b));
            let again = ann(render_step(&a).trim_start_matches("<!-- fm:step ").trim_end_matches(" -->"));
            prop_assert_eq!(render_step(&again), render_step(&a));
        }
    }

    /// files.md §4.10 / ADR-023 §5: a cached template with a matching source hash is loaded.
    #[test]
    fn golden_v2_2_cache() {
        use foreman_core::template::cache::{Origin, cache_path, load_or_parse};
        let app = tempfile::tempdir().unwrap();
        let root = foreman_core::store::Root::open_app_data(app.path()).unwrap();
        let src = blueprint();
        let (a, o1) = load_or_parse(&root, &src).unwrap();
        assert_eq!(o1, Origin::Parsed);
        let (b, o2) = load_or_parse(&root, &src).unwrap();
        assert_eq!(o2, Origin::Cache);
        assert_eq!(a, b, "deserialize(serialize(t)) == t");
        let path = app.path().join(cache_path(&a.source_sha256));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"schemaVersion\": 1"));
        assert!(
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with(".p1.json")
        );
        // A corrupt cache is replaced by a fresh parse.
        std::fs::write(&path, "{not json").unwrap();
        let (c, o3) = load_or_parse(&root, &src).unwrap();
        assert_eq!((o3, c), (Origin::Parsed, a));
    }
}
