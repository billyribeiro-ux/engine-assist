// PLANT (scripts/lint-plants): std::fs outside foreman-core::store must fail clippy (SR-31a).
/// Plant.
///
/// # Errors
/// Never in practice; this file exists only to trip the lint.
pub fn plant() -> std::io::Result<String> {
    std::fs::read_to_string("/etc/hosts")
}
