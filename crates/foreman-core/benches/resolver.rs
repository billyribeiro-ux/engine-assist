//! NFR-04 (resolution part) / AC-F006-16: prompt resolution p95 ≤ 50 ms, on the TradeLog fixture
//! (the real v2.2 blueprint, all conditions yes, signed packets for Stages 1–4).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::disallowed_methods,
    reason = "bench code"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use criterion::{Criterion, criterion_group, criterion_main};
use foreman_core::resolver::{PacketText, ResolveCtx, resolve};
use foreman_core::rules::applicable_path;
use foreman_core::schema::overlay::OverlayFile;
use foreman_core::schema::project::ProjectFile;
use foreman_core::template::parse_template;

fn fixture(rel: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(rel))
        .unwrap()
}

fn resolution(c: &mut Criterion) {
    let t = parse_template(&fixture("blueprint/claude-build-blueprint-v2.2.md"));
    let project: ProjectFile = serde_json::from_str(&fixture("tradelog/project.json")).unwrap();
    let path = applicable_path(&t, &project.conditions, &OverlayFile::empty());
    let packets: BTreeMap<u8, PacketText> = (1..=4u8)
        .map(|n| {
            let text = fixture(&format!("tradelog/handoff/stage-{n:02}.md"));
            (n, PacketText { text, signed: true })
        })
        .collect();
    let (inputs, remembered, optional, attested) =
        (BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeSet::new());
    let added = Vec::new();
    for key in ["2.2", "5.1"] {
        let ctx = ResolveCtx {
            template: &t,
            step: t.step(key).unwrap(),
            project: &project,
            path: &path,
            packets: &packets,
            inputs: &inputs,
            remembered: &remembered,
            optional: &optional,
            attested_na: &attested,
            overlay_items: &added,
        };
        assert!(resolve(&ctx).is_ok(), "{key}");
        c.bench_function(&format!("resolve_{}", key.replace('.', "_")), |b| {
            b.iter(|| std::hint::black_box(resolve(&ctx)));
        });
    }
}

criterion_group!(benches, resolution);
criterion_main!(benches);
