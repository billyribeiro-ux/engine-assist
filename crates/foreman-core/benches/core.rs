//! criterion benches (ADR-015 layer 6). M1/M2 add NFR-02/03/04; this one proves the wiring.
use criterion::{Criterion, criterion_group, criterion_main};

fn version(c: &mut Criterion) {
    c.bench_function("version", |b| {
        b.iter(|| std::hint::black_box(foreman_core::version()))
    });
}

criterion_group!(benches, version);
criterion_main!(benches);
