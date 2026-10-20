//! Renders annotations back to `fm:` comments in canonical key order (the round-trip property,
//! AC-F001-05): parse → render → parse is the identity on the values.

use super::grammar::{AltRule, ChainRef, InputType, Session, StepAnnotation, StepKind, Tag};

/// Renders an `fm:step` on one line, keys in DR-022 order, defaults included.
#[must_use]
pub fn render_step(a: &StepAnnotation) -> String {
    let mut parts = vec![format!("id={}", a.id)];
    if a.kind != StepKind::Prompt {
        parts.push(format!("kind={}", a.kind.as_str()));
    }
    if a.tag != Tag::Core {
        parts.push(format!("tag={}", a.tag.as_str()));
    }
    if let Some((src, _)) = &a.when {
        parts.push(format!("when={src}"));
    }
    if let Some(s) = a.surface {
        parts.push(format!("surface={}", s.as_str()));
    }
    if let Some(c) = a.cwd {
        parts.push(format!("cwd={}", c.as_str()));
    }
    if a.session == Session::Continue {
        parts.push("session=continue".into());
    }
    match &a.chain {
        Some(ChainRef::Step(id)) => parts.push(format!("chain={id}")),
        Some(ChainRef::Stage) => parts.push("chain=@stage".into()),
        None => {}
    }
    if let Some(s) = a.start {
        parts.push(format!("start={s}"));
    }
    for w in &a.switches {
        parts.push(format!("switch={}:{}", w.trigger, w.to));
    }
    for alt in &a.alts {
        let rule = match alt.rule {
            AltRule::ProPlan => "pro-plan",
            AltRule::MaxPlan => "max-plan",
        };
        parts.push(format!("alt={rule}:{}@{}", alt.model.as_str(), alt.effort.as_str()));
    }
    for i in &a.inputs {
        let ty = match i.kind {
            InputType::Text => "text",
            InputType::Multiline => "multiline",
        };
        parts.push(format!("in={}:{ty}{}", i.name, if i.remember { ":remember" } else { "" }));
    }
    if !a.out.is_empty() {
        parts.push(format!("out={}", a.out.join(",")));
    }
    if let Some(p) = &a.appends {
        parts.push(format!("appends={p}"));
    }
    if !a.requires.is_empty() {
        parts.push(format!("requires={}", a.requires.join(",")));
    }
    format!("<!-- fm:step {} -->", parts.join(" "))
}
