// PLANT (scripts/lint-plants): Command::new outside foreman-sys::spawn must fail clippy (SR-02).
pub fn plant() -> std::io::Result<std::process::ExitStatus> {
    std::process::Command::new("/bin/sh").arg("-c").arg("true").status()
}
