#![allow(clippy::unwrap_used, clippy::disallowed_methods, clippy::print_stdout, missing_docs)]
fn main() {
    let p = std::env::args().nth(1).unwrap();
    let t = foreman_core::template::parse_template(&std::fs::read_to_string(p).unwrap());
    for d in &t.diagnostics { println!("{:?} {}:{} {} {} (stage {:?})", d.severity, d.line, d.column, d.rule, d.message, d.stage); }
    for s in &t.steps { if s.stage <= 5 || s.template_id.is_some() && s.stage <= 6 { println!("{} {:?} {:?} {:?} {:?} {:?} chain={:?} start={:?} out={:?} parts={}", s.key, s.status, s.kind, s.surface, s.cwd, s.session, s.chain.as_ref().map(|c| c.to_string()), s.start.map(|x| x.to_string()), s.out, s.prompt.as_ref().map_or(0, |p| p.parts.len())); } }
    println!("stages={} steps={} items={} formats={:?}", t.stages.len(), t.steps.len(), t.items.len(), t.packet_formats.iter().map(|f| (f.stage, f.keys.len())).collect::<Vec<_>>());
}
