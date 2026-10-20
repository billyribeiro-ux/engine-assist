//! M1-03, M1-04, M1-06: the lexical path policy, the no-symlink walk and read policy, reads outside
//! projects and the secret-file exclusion (ADR-011, SR-30, SR-31, SR-31a, SR-50, SR-53).
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

use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::Path;

use foreman_core::pathpolicy::{
    self, DecisionPath, PolicyViolation, ReadClass, WritePurpose, check_read, check_write,
};
use foreman_core::store::{ReadStamp, Root, RootKind, StoreError};

fn purpose(s: &str) -> WritePurpose {
    match s {
        "repo" => WritePurpose::Repo,
        "template-out" => WritePurpose::TemplateOut,
        "assembler" => WritePurpose::AssemblerClaudeMd,
        "decision:project" => WritePurpose::DecisionFile(DecisionPath::Project),
        "decision:stage-02" => WritePurpose::DecisionFile(DecisionPath::StageRecord(2)),
        s if s.starts_with("council:") => WritePurpose::CouncilSetup(s[8..].parse().unwrap()),
        other => panic!("purpose {other}"),
    }
}

fn rule(r: Result<pathpolicy::RepoPath, PolicyViolation>) -> String {
    match r {
        Ok(_) => "ok".into(),
        Err(v) => v.rule().into(),
    }
}

fn project() -> (tempfile::TempDir, Root) {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("docs/handoff")).unwrap();
    let r = Root::open(d.path(), RootKind::Project).unwrap();
    (d, r)
}

fn rp(s: &str) -> pathpolicy::RepoPath {
    check_read(s, ReadClass::Decision).unwrap()
}

mod fs_policy {
    use super::*;
    use proptest::prelude::*;

    /// AC-F021-01: every golden case is refused with the rule that refused it (or accepted).
    #[test]
    fn golden_suite() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(&common::read_fixture("fixtures/fs_policy_golden.json")).unwrap();
        let mut failures = Vec::new();
        for c in &cases {
            let path = c["path"].as_str().unwrap();
            let got = rule(check_write(path, purpose(c["purpose"].as_str().unwrap())));
            if got != c["expect"].as_str().unwrap() {
                failures.push(format!(
                    "{path:?} ({}) → {got}, expected {}",
                    c["purpose"], c["expect"]
                ));
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
        // A REPO FILES list containing each case (as pasted from a Stage 4 packet): every
        // non-ok line is refused, one rule each.
        let repo_files: Vec<&str> = cases
            .iter()
            .filter(|c| c["purpose"] == "repo")
            .map(|c| c["path"].as_str().unwrap())
            .collect();
        let refused = repo_files
            .iter()
            .filter(|p| check_write(p, WritePurpose::Repo).is_err())
            .count();
        let expected = cases
            .iter()
            .filter(|c| c["purpose"] == "repo" && c["expect"] != "ok")
            .count();
        assert_eq!(refused, expected);
    }

    /// AC-F021-04 (golden part): a symlinked `docs/handoff` and a symlinked final file are refused
    /// by the directory-handle walk.
    #[test]
    fn golden_symlinked_handoff() {
        let (d, _) = project();
        let outside = tempfile::tempdir().unwrap();
        std::fs::remove_dir(d.path().join("docs/handoff")).unwrap();
        symlink(outside.path(), d.path().join("docs/handoff")).unwrap();
        let root = Root::open(d.path(), RootKind::Project).unwrap();
        let p = check_write("docs/handoff/stage-02.md", WritePurpose::Repo).unwrap();
        let e = root.write(&p, b"x", &ReadStamp::Absent).unwrap_err();
        assert_eq!(
            e,
            StoreError::Policy(PolicyViolation::Symlink("handoff".into()))
        );
        assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
        // Symlinked final file.
        std::fs::create_dir_all(d.path().join("spec")).unwrap();
        std::fs::write(outside.path().join("target.md"), "secret").unwrap();
        symlink(outside.path().join("target.md"), d.path().join("spec/x.md")).unwrap();
        let p = check_write("spec/x.md", WritePurpose::Repo).unwrap();
        let e = root.write(&p, b"x", &ReadStamp::Absent).unwrap_err();
        assert!(
            matches!(e, StoreError::Policy(PolicyViolation::Symlink(_))),
            "{e:?}"
        );
        assert_eq!(
            std::fs::read_to_string(outside.path().join("target.md")).unwrap(),
            "secret"
        );
    }

    /// AC-F021-03: the only exceptions to the deny rules are the council-setup destinations and the
    /// decision-file writers (and root CLAUDE.md through the assembler).
    #[test]
    fn allowlist_only() {
        for (i, agent) in pathpolicy::COUNCIL_AGENTS.iter().enumerate() {
            let p = format!("spec/.claude/agents/{agent}.md");
            assert!(check_write(&p, WritePurpose::CouncilSetup(i)).is_ok());
            assert!(check_write(&p, WritePurpose::Repo).is_err());
            assert!(check_write(&p, WritePurpose::TemplateOut).is_err());
        }
        assert!(check_write("spec/.claude/agents/x.md", WritePurpose::CouncilSetup(7)).is_err());
        assert!(
            check_write(
                "docs/.claude/agents/red-team.md",
                WritePurpose::CouncilSetup(5)
            )
            .is_err()
        );
        for d in [
            DecisionPath::Project,
            DecisionPath::Overlay,
            DecisionPath::Lock,
            DecisionPath::StageRecord(5),
        ] {
            assert!(check_write(&d.path(), WritePurpose::DecisionFile(d)).is_ok());
            assert!(check_write(&d.path(), WritePurpose::Repo).is_err());
        }
        assert!(
            check_write(
                ".foreman/other.json",
                WritePurpose::DecisionFile(DecisionPath::Project)
            )
            .is_err()
        );
        for top in [
            "src/x.rs",
            "package.json",
            "Cargo.toml",
            "README.md",
            "docs",
            "spec",
        ] {
            assert_eq!(
                rule(check_write(top, WritePurpose::Repo)),
                "path.not-allowlisted",
                "{top}"
            );
        }
    }

    #[test]
    fn rejects_overlong_path() {
        let ok = format!("docs/{}", "a".repeat(255));
        assert!(check_write(&ok, WritePurpose::Repo).is_ok());
        let long_comp = format!("docs/{}", "a".repeat(256));
        assert_eq!(
            check_write(&long_comp, WritePurpose::Repo),
            Err(PolicyViolation::ComponentTooLong(256))
        );
        let mut long = String::from("docs");
        while long.len() <= 1024 {
            long.push_str("/abcdefgh");
        }
        assert!(matches!(
            check_write(&long, WritePurpose::Repo),
            Err(PolicyViolation::TooLong(_))
        ));
        assert!(matches!(
            check_read(&long, ReadClass::Decision),
            Err(PolicyViolation::TooLong(_))
        ));
    }

    /// AC-F021-08 (lexical part, and the store on this volume): NFC + full case folding.
    #[test]
    fn case_and_normalization_variants() {
        let variants = [
            ".GIT",
            ".Git",
            ".gIT",
            ".CLAUDE",
            ".Claude",
            ".HUSKY",
            ".hus\u{212a}y",
            ".VSCode",
            ".IDEA",
            ".GitHub",
            ".FOREMAN",
            ".Foreman",
        ];
        for v in variants {
            for p in [
                format!("{v}/x"),
                format!("docs/{v}/x"),
                format!("spec/a/{v}/b.md"),
            ] {
                assert!(
                    matches!(
                        check_write(&p, WritePurpose::Repo),
                        Err(PolicyViolation::DeniedComponent(_))
                    ),
                    "{p}"
                );
            }
        }
        for n in [
            ".MCP.json",
            ".Mcp.Json",
            "claude.LOCAL.md",
            ".ENVRC",
            ".GitModules",
            ".GITATTRIBUTES",
            ".NPMRC",
        ] {
            assert!(
                matches!(
                    check_write(&format!("docs/{n}"), WritePurpose::Repo),
                    Err(PolicyViolation::DeniedName(_))
                ),
                "{n}"
            );
        }
        for n in ["CLAUDE.MD", "Claude.md", "claude.md", "CLAUDE.md"] {
            assert_eq!(
                check_write(&format!("docs/{n}"), WritePurpose::Repo),
                Err(PolicyViolation::ClaudeMd),
                "{n}"
            );
        }
        // NFD forms fold like NFC: a decomposed name equals its composed form.
        assert_eq!(pathpolicy::key("e\u{301}"), pathpolicy::key("\u{e9}"));
        // On this volume, the store refuses a planted `.Git` reached by its exact name too.
        let (d, root) = project();
        std::fs::create_dir_all(d.path().join("docs/.Git")).unwrap();
        let p = check_read("docs/.Git/x", ReadClass::Decision).unwrap();
        assert!(matches!(
            root.write(&p, b"x", &ReadStamp::Absent),
            Err(StoreError::Policy(PolicyViolation::DeniedComponent(_)))
        ));
        case_insensitive_volume();
    }

    /// The case-insensitive APFS volume check runs on macOS (a disk image created for the test).
    #[cfg(target_os = "macos")]
    fn case_insensitive_volume() {
        let d = tempfile::tempdir().unwrap();
        let img = d.path().join("ci.dmg");
        let ok = std::process::Command::new("/usr/bin/hdiutil")
            .args(["create", "-size", "8m", "-fs", "APFS", "-volname", "fmci"])
            .arg(&img)
            .status()
            .unwrap()
            .success();
        assert!(ok);
        let mnt = d.path().join("mnt");
        std::fs::create_dir(&mnt).unwrap();
        assert!(
            std::process::Command::new("/usr/bin/hdiutil")
                .args(["attach", "-nobrowse", "-mountpoint"])
                .arg(&mnt)
                .arg(&img)
                .status()
                .unwrap()
                .success()
        );
        std::fs::create_dir_all(mnt.join("docs/.Git")).unwrap();
        let root = Root::open(&mnt, RootKind::Project).unwrap();
        // `docs/xgit` doesn't exist; the variant `docs/.gIt` is denied lexically, and the reported
        // name check catches the existing `.Git` reached as `.git` would be (both refused).
        let p = check_read("docs/.gIt/x", ReadClass::Decision).unwrap();
        assert!(root.write(&p, b"x", &ReadStamp::Absent).is_err());
        let _ = std::process::Command::new("/usr/bin/hdiutil")
            .args(["detach"])
            .arg(&mnt)
            .status();
    }

    #[cfg(not(target_os = "macos"))]
    fn case_insensitive_volume() {
        // Linux CI has no case-folding volume (tmpfs casefold needs CONFIG_UNICODE); the walk's use
        // of the reported name is proven by `reported_name_checked` on every platform.
    }

    /// AC-F021-08: the name the file system reports for an existing entry is checked again. On a
    /// case-insensitive volume, `docs/notes` can open an entry stored as `.Git`; the reporter hook
    /// simulates that volume here, and the walk refuses it.
    #[test]
    fn reported_name_checked() {
        let (d, _) = project();
        std::fs::create_dir_all(d.path().join("docs/notes")).unwrap();
        std::fs::write(d.path().join("docs/notes/a.md"), "x").unwrap();
        fn as_if_insensitive(n: &str) -> String {
            if n == "notes" {
                ".Git".into()
            } else {
                n.into()
            }
        }
        let root = Root::open(d.path(), RootKind::Project)
            .unwrap()
            .with_name_reporter(as_if_insensitive);
        let p = rp("docs/notes/a.md");
        assert_eq!(
            root.read(&p, ReadClass::Decision).unwrap_err(),
            StoreError::Policy(PolicyViolation::DeniedComponent(".Git".into()))
        );
        assert!(root.write(&p, b"y", &ReadStamp::Absent).is_err());
        // A reported final name is checked as well.
        fn final_name(n: &str) -> String {
            if n == "a.md" {
                ".MCP.json".into()
            } else {
                n.into()
            }
        }
        let root = Root::open(d.path(), RootKind::Project)
            .unwrap()
            .with_name_reporter(final_name);
        assert!(matches!(
            root.read(&p, ReadClass::Decision),
            Err(StoreError::Policy(PolicyViolation::DeniedName(_)))
        ));
    }

    /// AC-XC-10: `~/.claude/settings.json`, `~/.claude.json` and `.claude/settings*.json` are never
    /// writable.
    #[test]
    fn denies_claude_settings_writes() {
        for p in [
            "~/.claude/settings.json",
            "/Users/billy/.claude/settings.json",
            "/Users/billy/.claude.json",
            ".claude/settings.json",
            ".claude/settings.local.json",
            "spec/.claude/settings.json",
            "docs/.claude/settings.local.json",
            "spec/.Claude/Settings.json",
        ] {
            for purpose in [
                WritePurpose::Repo,
                WritePurpose::TemplateOut,
                WritePurpose::AssemblerClaudeMd,
                WritePurpose::CouncilSetup(0),
                WritePurpose::DecisionFile(DecisionPath::Project),
            ] {
                assert!(check_write(p, purpose).is_err(), "{p} {purpose:?}");
            }
        }
    }

    /// AC-F021-07: a symlink at any component of a read is refused (`O_NOFOLLOW` on the last).
    #[test]
    fn read_refuses_symlink_at_any_component() {
        let (d, _) = project();
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(outside.path().join("b/c")).unwrap();
        std::fs::write(outside.path().join("b/c/stage-01.md"), "outside").unwrap();
        std::fs::write(outside.path().join("f.md"), "outside").unwrap();
        // first, middle and last components
        symlink(outside.path(), d.path().join("spec")).unwrap();
        std::fs::create_dir_all(d.path().join("docs/a")).unwrap();
        symlink(outside.path().join("b"), d.path().join("docs/a/b")).unwrap();
        symlink(outside.path().join("f.md"), d.path().join("docs/f.md")).unwrap();
        let root = Root::open(d.path(), RootKind::Project).unwrap();
        for (p, comp) in [
            ("spec/f.md", "spec"),
            ("docs/a/b/c/stage-01.md", "b"),
            ("docs/f.md", "f.md"),
        ] {
            assert_eq!(
                root.read(&rp(p), ReadClass::Decision).unwrap_err(),
                StoreError::Policy(PolicyViolation::Symlink(comp.into())),
                "{p}"
            );
        }
    }

    #[test]
    fn read_refuses_hardlink() {
        let (d, root) = project();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("id_ed25519"), "key").unwrap();
        // Same filesystem is needed for a hard link: link inside the project.
        std::fs::write(d.path().join("docs/real.md"), "x").unwrap();
        std::fs::hard_link(
            d.path().join("docs/real.md"),
            d.path().join("docs/handoff/stage-01.md"),
        )
        .unwrap();
        assert_eq!(
            root.read(&rp("docs/handoff/stage-01.md"), ReadClass::Decision)
                .unwrap_err(),
            StoreError::HardLink("docs/handoff/stage-01.md".into(), 2)
        );
        let p = check_write("docs/handoff/stage-01.md", WritePurpose::Repo).unwrap();
        let stamp = ReadStamp::Absent;
        assert!(matches!(
            root.write(&p, b"y", &stamp),
            Err(StoreError::HardLink(_, 2))
        ));
        assert_eq!(
            std::fs::read_to_string(d.path().join("docs/real.md")).unwrap(),
            "x"
        );
    }

    /// AC-F013-16: larger than 1 MB, not a regular file, more than one link → refused with why.
    #[test]
    fn read_size_cap() {
        let (d, root) = project();
        std::fs::write(
            d.path().join("docs/handoff/stage-02.md"),
            vec![b'a'; 1024 * 1024 + 1],
        )
        .unwrap();
        let e = root
            .read(&rp("docs/handoff/stage-02.md"), ReadClass::Decision)
            .unwrap_err();
        assert!(e.to_string().contains("over the 1048576-byte limit"), "{e}");
        std::fs::write(d.path().join("docs/lessons.md"), vec![b'a'; 1024 * 1024]).unwrap();
        assert!(
            root.read(&rp("docs/lessons.md"), ReadClass::Decision)
                .is_ok()
        );
        std::fs::create_dir_all(d.path().join("spec")).unwrap();
        std::fs::write(
            d.path().join("spec/settings.json"),
            vec![b' '; 256 * 1024 + 1],
        )
        .unwrap();
        assert!(matches!(
            root.read(&rp("spec/settings.json"), ReadClass::ClaudeConfig),
            Err(StoreError::TooLarge { .. })
        ));
        // A FIFO doesn't block and is refused.
        let fifo = d.path().join("docs/handoff/stage-03.md");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        assert_eq!(
            root.read(&rp("docs/handoff/stage-03.md"), ReadClass::Decision)
                .unwrap_err(),
            StoreError::NotRegular("docs/handoff/stage-03.md".into())
        );
        // A directory is not a regular file.
        assert!(matches!(
            root.read(&rp("docs/handoff"), ReadClass::Decision),
            Err(StoreError::NotRegular(_))
        ));
    }

    /// AC-XC-30: outside reads are a fixed list in one module; everything else is unreachable.
    #[test]
    fn read_scope() {
        use foreman_core::store::outside::{self, OutsideRead, UserClaudeEntry};
        let home = tempfile::tempdir().unwrap();
        let h = home.path();
        std::fs::create_dir_all(h.join(".claude/agents")).unwrap();
        std::fs::create_dir_all(h.join(".ssh")).unwrap();
        std::fs::write(h.join(".claude/settings.json"), r#"{"env":{"A":"1"}}"#).unwrap();
        std::fs::write(h.join(".claude/agents/x.md"), "agent").unwrap();
        std::fs::write(h.join(".ssh/id_ed25519"), "KEY").unwrap();
        std::fs::write(
            h.join(".claude.json"),
            r#"{"oauthAccount":{"email":"x"},"mcpServers":{"a":{"command":"x"}},"projects":{"/p":{"history":["secret"],"mcpServers":{}}}}"#,
        )
        .unwrap();
        assert_eq!(
            outside::read(&OutsideRead::UserSettings, h, None).unwrap(),
            br#"{"env":{"A":"1"}}"#
        );
        assert_eq!(
            outside::read(
                &OutsideRead::UserClaude(UserClaudeEntry::Agents, Some("x.md".into())),
                h,
                None
            )
            .unwrap(),
            b"agent"
        );
        // ~/.claude.json: only mcpServers survives the read.
        let j =
            String::from_utf8(outside::read(&OutsideRead::ClaudeJsonMcpServers, h, None).unwrap())
                .unwrap();
        assert!(
            j.contains("mcpServers") && !j.contains("oauthAccount") && !j.contains("history"),
            "{j}"
        );
        // Dynamic names can't climb or reach secrets.
        for bad in ["../../.ssh/id_ed25519", ".env", "a/b.md", "id_rsa"] {
            assert!(
                outside::read(
                    &OutsideRead::UserClaude(UserClaudeEntry::Agents, Some(bad.into())),
                    h,
                    None
                )
                .is_err(),
                "{bad}"
            );
        }
        // The list is exactly AC-XC-30's.
        let all = [
            OutsideRead::UserSettings,
            OutsideRead::ManagedSettings,
            OutsideRead::ManagedDropIn("a.json".into()),
            OutsideRead::ClaudeJsonMcpServers,
            OutsideRead::UserClaude(UserClaudeEntry::ClaudeMd, None),
            OutsideRead::AncestorClaudeMd {
                dir: "/Users/billy/dev".into(),
                local: true,
            },
            OutsideRead::EtcPaths,
            OutsideRead::EtcPathsD("homebrew".into()),
            OutsideRead::EtcShells,
            OutsideRead::BlueprintCandidate(0),
        ];
        let locations: Vec<String> = all
            .iter()
            .map(|r| {
                let (b, rel) = outside::location(r, Path::new("/Users/billy"), None).unwrap();
                b.join(rel).display().to_string()
            })
            .collect();
        insta::assert_snapshot!(locations.join("\n"));
        // Transcript existence is a stat, never an open.
        assert!(outside::transcript_exists(&h.join(".claude/settings.json")));
        assert!(!outside::transcript_exists(&h.join("nope.jsonl")));
        // AC-XC-30's clippy part: std::fs is denied outside store (scripts/lint-plants proves it).
        let clippy = std::fs::read_to_string(common::repo_root().join("clippy.toml")).unwrap();
        assert!(clippy.contains("std::fs::read") && clippy.contains("foreman-core::store"));
    }

    proptest! {
        /// AC-F021-02: denied components at any depth, NUL, controls and bidi are refused.
        #[test]
        fn denied_at_any_depth(depth in 0usize..6, deny in 0usize..7, upper in any::<bool>(), extra in "[a-z]{1,8}") {
            let mut comps: Vec<String> = vec!["docs".into()];
            for _ in 0..depth { comps.push(extra.clone()); }
            let mut d = pathpolicy::DENIED_COMPONENTS[deny].to_owned();
            if upper { d = d.to_uppercase(); }
            comps.push(d);
            comps.push("x.md".into());
            prop_assert!(check_write(&comps.join("/"), WritePurpose::Repo).is_err());
        }

        #[test]
        fn control_and_bidi_refused(prefix in "[a-z]{1,8}", c in prop_oneof![0u32..0x20, Just(0x7f), 0x80u32..0xa0, 0x202au32..0x202f, 0x2066u32..0x206a, 0x200bu32..0x200e]) {
            let ch = char::from_u32(c).unwrap();
            let p = format!("docs/{prefix}{ch}.md");
            prop_assert!(check_write(&p, WritePurpose::Repo).is_err());
            prop_assert!(check_read(&p, ReadClass::Decision).is_err());
        }

        /// AC-F021-05: any accepted generated path resolves inside the root (checked by inode:
        /// nothing outside changes), even with planted symlinks.
        #[test]
        fn accepted_paths_stay_inside(segs in proptest::collection::vec("[a-z.]{1,6}", 1..4), plant in any::<bool>()) {
            let (d, _) = project();
            let outside = tempfile::tempdir().unwrap();
            if plant {
                std::fs::create_dir_all(d.path().join("docs")).unwrap();
                let _ = symlink(outside.path(), d.path().join("docs").join(&segs[0]));
            }
            let root = Root::open(d.path(), RootKind::Project).unwrap();
            let rel = format!("docs/{}", segs.join("/"));
            if let Ok(p) = check_write(&rel, WritePurpose::Repo) {
                let _ = root.write(&p, b"x", &ReadStamp::Absent);
            }
            prop_assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
        }
    }
}

mod secrets {
    use super::*;

    /// AC-XC-11 / AB-08: secret patterns (and their case variants) are never opened, listed or watched.
    #[test]
    fn excluded_from_views_and_watcher() {
        let secrets = [
            ".env",
            ".env.local",
            ".ENV.production",
            "docs/.Env",
            "server.pem",
            "docs/tls.PEM",
            "spec/private.key",
            "id_rsa",
            "docs/ID_ED25519.pub",
            "cert.p12",
            "docs/.npmrc",
            ".NETRC",
            ".ssh/config",
            "docs/.SSH/known_hosts",
            ".aws/credentials",
            "docs/.Aws/config",
            ".config/gcloud/creds.json",
            "docs/.Config/GCloud/x",
        ];
        let (d, root) = project();
        for s in secrets {
            assert!(pathpolicy::is_secret(s), "{s} (watcher filter)");
            assert!(
                matches!(
                    check_read(s, ReadClass::Decision),
                    Err(PolicyViolation::Secret(_))
                ),
                "{s}"
            );
            let full = d.path().join(s);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(&full, "TOKEN").unwrap();
        }
        for ok in [
            "docs/environment.md",
            "docs/keys.md",
            "docs/handoff/stage-01.md",
            "spec/identity.md",
        ] {
            assert!(!pathpolicy::is_secret(ok), "{ok}");
            assert!(check_read(ok, ReadClass::Decision).is_ok(), "{ok}");
        }
        // Even a reported name matching a secret pattern is refused by the store.
        std::fs::write(d.path().join("docs/notes.md"), "x").unwrap();
        fn as_env(n: &str) -> String {
            if n == "notes.md" {
                ".env".into()
            } else {
                n.into()
            }
        }
        let r = Root::open(d.path(), RootKind::Project)
            .unwrap()
            .with_name_reporter(as_env);
        assert!(matches!(
            r.read(&rp("docs/notes.md"), ReadClass::Decision),
            Err(StoreError::Policy(PolicyViolation::Secret(_)))
        ));
        drop(root);
    }

    #[test]
    fn symlink_to_secret_refused() {
        let (d, root) = project();
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join(".ssh")).unwrap();
        std::fs::write(home.path().join(".ssh/id_ed25519"), "PRIVATE KEY").unwrap();
        symlink(
            home.path().join(".ssh/id_ed25519"),
            d.path().join("docs/handoff/stage-02.md"),
        )
        .unwrap();
        symlink(home.path().join(".ssh"), d.path().join("docs/keys")).unwrap();
        let e = root
            .read(&rp("docs/handoff/stage-02.md"), ReadClass::Decision)
            .unwrap_err();
        assert!(matches!(e, StoreError::Policy(PolicyViolation::Symlink(_))));
        let e = root
            .read(&rp("docs/keys/config"), ReadClass::Decision)
            .unwrap_err();
        assert!(matches!(e, StoreError::Policy(PolicyViolation::Symlink(_))));
        // Outside reads refuse it too.
        let h = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(h.path().join(".claude")).unwrap();
        symlink(
            home.path().join(".ssh/id_ed25519"),
            h.path().join(".claude/settings.json"),
        )
        .unwrap();
        assert!(
            foreman_core::store::outside::read(
                &foreman_core::store::outside::OutsideRead::UserSettings,
                h.path(),
                None
            )
            .is_err()
        );
    }
}

mod fs {
    use super::*;

    fn walk(dir: &Path, out: &mut Vec<(String, u32, bool)>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let e = e.unwrap();
            let md = std::fs::symlink_metadata(e.path()).unwrap();
            out.push((
                e.path().display().to_string(),
                md.permissions().mode() & 0o777,
                md.is_dir(),
            ));
            if md.is_dir() {
                walk(&e.path(), out);
            }
        }
    }

    /// AC-XC-14 / SR-53: app-data folders 0700, files 0600 from the first write, whatever the umask.
    #[test]
    fn app_data_permissions() {
        let base = tempfile::tempdir().unwrap();
        let app = base.path().join("Application Support/dev.foreman");
        let root = Root::open_app_data(&app).unwrap();
        assert_eq!(
            std::fs::metadata(&app).unwrap().permissions().mode() & 0o777,
            0o700
        );
        for p in [
            "settings.json",
            "projects/abc/progress.json",
            "runs/r1/run.json",
            "approvals/abc/project.json",
        ] {
            root.write(
                &pathpolicy::check_app_data(p).unwrap(),
                b"{}\n",
                &ReadStamp::Absent,
            )
            .unwrap();
        }
        root.append_line(
            &pathpolicy::check_app_data("runs/index-2026-10.jsonl").unwrap(),
            b"{}\n",
        )
        .unwrap();
        let mut all = Vec::new();
        walk(&app, &mut all);
        assert!(all.len() >= 9);
        for (p, mode, is_dir) in all {
            assert_eq!(mode, if is_dir { 0o700 } else { 0o600 }, "{p}");
        }
    }
}
