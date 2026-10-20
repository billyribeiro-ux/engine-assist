//! NFR-03 / NFR-24 / AC-F001-07: template load (parse + validate + lint) on the v2.2 blueprint
//! and on a 20,000-line blueprint, uncached and cached. Targets: p95 ≤ 50 ms uncached, ≤ 5 ms
//! cached.
//!
//! Measurement note (2026-10-20): the cached 20,000-line load measured 7.4 ms against the 5 ms
//! target on a shared 2-core Xeon @ 2.8 GHz Linux container, which is not the target hardware.
//! The target is unchanged. NFR-03 is verified by running this bench on Billy's Mac (task in
//! docs/progress.md, "Performance measurements still owed"); do not tune against the Linux
//! figure.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::disallowed_methods,
    reason = "bench code"
)]

use std::path::Path;

use criterion::{Criterion, criterion_group, criterion_main};
use foreman_core::store::Root;
use foreman_core::template::{cache, parse_template};

fn v22() -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/blueprint/claude-build-blueprint-v2.2.md"),
    )
    .unwrap()
}

/// The v2.2 blueprint grown to 20,000 lines with unannotated prose, lists and fences in its last
/// stage (NFR-24): built here, deterministically, rather than committed.
fn blueprint_20k() -> String {
    let mut s = v22();
    let mut n = s.lines().count();
    let mut i = 0usize;
    while n < 20_000 {
        let chunk = format!(
            "\n### Appendix note {i}\n\nPlain prose paragraph {i} about the build, with `code`, **bold** and a [link](https://example.com/{i}).\nA second line of the same paragraph.\n\n- bullet one of {i}\n- bullet two of {i}\n  continued\n\n```markdown\nexample block {i}\n```\n"
        );
        n += chunk.matches('\n').count();
        s.push_str(&chunk);
        i += 1;
    }
    s
}

fn load(c: &mut Criterion) {
    let src = v22();
    let big = blueprint_20k();
    assert!(big.lines().count() >= 20_000);
    c.bench_function("template_load_v2_2_uncached", |b| {
        b.iter(|| std::hint::black_box(parse_template(&src)));
    });
    c.bench_function("template_load_20k_uncached", |b| {
        b.iter(|| std::hint::black_box(parse_template(&big)));
    });
    let app = tempfile::tempdir().unwrap();
    let root = Root::open_app_data(app.path()).unwrap();
    let (_, first) = cache::load_or_parse(&root, &big).unwrap();
    assert_eq!(first, cache::Origin::Parsed);
    c.bench_function("template_load_20k_cached", |b| {
        b.iter(|| {
            let (t, o) = cache::load_or_parse(&root, &big).unwrap();
            assert_eq!(o, cache::Origin::Cache);
            std::hint::black_box(t)
        });
    });
}

criterion_group!(benches, load);
criterion_main!(benches);
