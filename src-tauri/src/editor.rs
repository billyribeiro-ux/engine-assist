//! The editor setting's guard (security review 2026-10-20, W-1; T-004, SR-24a, SR-25a).
//!
//! `file_open` runs `/usr/bin/open -b <editor> <file>`. An app that runs what it opens (a
//! terminal, a script runner) would turn Open in editor into running a repo file, so those apps are
//! refused as the editor, and any other change of editor is applied only after a native dialog
//! that names the app.
//!
//! The names come from a fixed table of common editors: Foreman resolves no bundle ID through the
//! OS here, because that would need a program or an app-folder read outside SR-01 and SR-31a's
//! fixed lists ([CR-016](../../docs/change-requests/CR-016-editor-app-name-from-launch-services.md)
//! proposes it). An ID outside the table is shown as the ID, with a line saying so.

/// Apps that run what they open, compared without case (`LaunchServices` treats bundle IDs
/// case-insensitively). A prefix entry ends with `.` and covers every ID under it.
pub const REFUSED: [(&str, &str); 14] = [
    ("com.apple.Terminal", "Terminal"),
    ("com.googlecode.iterm2", "iTerm2"),
    ("com.apple.ScriptEditor2", "Script Editor"),
    ("com.apple.ScriptEditor.id.", "an AppleScript applet"),
    ("com.apple.Automator", "Automator"),
    ("com.apple.automator.", "an Automator app"),
    ("com.apple.shortcuts", "Shortcuts"),
    ("org.python.PythonLauncher", "Python Launcher"),
    ("net.kovidgoyal.kitty", "kitty"),
    ("org.alacritty", "Alacritty"),
    ("com.github.wez.wezterm", "WezTerm"),
    ("dev.warp.", "Warp"),
    ("com.mitchellh.ghostty", "Ghostty"),
    ("co.zeit.hyper", "Hyper"),
];

/// Common editors' display names.
pub const KNOWN: [(&str, &str); 9] = [
    ("com.microsoft.VSCode", "Visual Studio Code"),
    ("com.todesktop.230313mzl4w4u92", "Cursor"),
    ("dev.zed.Zed", "Zed"),
    ("com.sublimetext.4", "Sublime Text"),
    ("com.apple.TextEdit", "TextEdit"),
    ("com.barebones.bbedit", "BBEdit"),
    ("com.panic.Nova", "Nova"),
    ("com.coteditor.CotEditor", "CotEditor"),
    ("com.apple.dt.Xcode", "Xcode"),
];

fn matches(id: &str, entry: &str) -> bool {
    let id = id.to_ascii_lowercase();
    let entry = entry.to_ascii_lowercase();
    if entry.ends_with('.') {
        id.starts_with(&entry)
    } else {
        id == entry
    }
}

/// The refused app's name when `bundle_id` is an app that runs what it opens.
#[must_use]
pub fn refused(bundle_id: &str) -> Option<&'static str> {
    REFUSED
        .iter()
        .find(|(id, _)| matches(bundle_id, id))
        .map(|(_, name)| *name)
}

/// A known editor's name.
#[must_use]
pub fn known_name(bundle_id: &str) -> Option<&'static str> {
    KNOWN
        .iter()
        .find(|(id, _)| matches(bundle_id, id))
        .map(|(_, name)| *name)
}

/// What the dialog calls the app: its name and ID, or the ID alone.
#[must_use]
pub fn shown(bundle_id: &str) -> String {
    known_name(bundle_id).map_or_else(|| bundle_id.to_owned(), |n| format!("{n} ({bundle_id})"))
}

/// The refusal's sentence.
#[must_use]
pub fn refusal(name: &str) -> String {
    format!(
        "{name} runs the files it opens, so Foreman won't use it as the editor. Choose a text editor."
    )
}

/// The native dialog's lines for a new editor.
#[must_use]
pub fn confirm_lines(bundle_id: &str) -> Vec<String> {
    let mut out = vec![format!(
        "Open files from Open in editor with {}.",
        shown(bundle_id)
    )];
    if known_name(bundle_id).is_none() {
        out.push(
            "Foreman doesn't know this app ID. Make sure it is a text editor: Open in editor hands it project files."
                .to_owned(),
        );
    }
    out
}
