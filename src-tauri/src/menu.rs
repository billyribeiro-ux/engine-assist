//! The macOS app menu, built in Rust (security review 2026-10-20, C-1; ADR-006 amendment).
//!
//! Foreman builds its own menu instead of Tauri's default, so what each item does is decided here
//! and pinned by a test. Every item is a native (predefined) one. Edit → Paste is the native
//! `paste:` action: in a text field it pastes as usual; with a terminal focused, WebKit turns it into
//! a DOM `paste` event that `TerminalView` stops in the capture phase and answers with a Rust paste
//! (`pty_paste` reads the clipboard and removes every control, SR-28), the same path as ⌘V. The
//! event's own text never reaches the PTY. There is no "Paste and Match Style".

use tauri::menu::{Menu, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Runtime};

/// One native menu item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    /// About Foreman.
    About,
    /// Services.
    Services,
    /// Hide Foreman.
    Hide,
    /// Hide Others.
    HideOthers,
    /// Show All.
    ShowAll,
    /// Quit Foreman (⌘Q goes through the quit guard, AC-F009-09).
    Quit,
    /// Undo.
    Undo,
    /// Redo.
    Redo,
    /// Cut.
    Cut,
    /// Copy.
    Copy,
    /// Paste: the native `paste:` action (see the module docs).
    Paste,
    /// Select All.
    SelectAll,
    /// Enter Full Screen.
    Fullscreen,
    /// Minimize.
    Minimize,
    /// Zoom.
    Zoom,
    /// Close Window (goes through the quit guard with live sessions).
    CloseWindow,
    /// A separator.
    Separator,
}

/// A submenu: its title and items, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Section {
    /// The title (the app menu's title is replaced by the app name on macOS).
    pub title: &'static str,
    /// The items.
    pub items: &'static [Item],
}

/// The whole menu bar.
pub const MENU: [Section; 5] = [
    Section {
        title: "Foreman",
        items: &[
            Item::About,
            Item::Separator,
            Item::Services,
            Item::Separator,
            Item::Hide,
            Item::HideOthers,
            Item::ShowAll,
            Item::Separator,
            Item::Quit,
        ],
    },
    Section {
        title: "File",
        items: &[Item::CloseWindow],
    },
    Section {
        title: "Edit",
        items: &[
            Item::Undo,
            Item::Redo,
            Item::Separator,
            Item::Cut,
            Item::Copy,
            Item::Paste,
            Item::SelectAll,
        ],
    },
    Section {
        title: "View",
        items: &[Item::Fullscreen],
    },
    Section {
        title: "Window",
        items: &[
            Item::Minimize,
            Item::Zoom,
            Item::Separator,
            Item::CloseWindow,
        ],
    },
];

fn native<R: Runtime>(h: &AppHandle<R>, item: Item) -> tauri::Result<PredefinedMenuItem<R>> {
    match item {
        Item::About => PredefinedMenuItem::about(h, None, None),
        Item::Services => PredefinedMenuItem::services(h, None),
        Item::Hide => PredefinedMenuItem::hide(h, None),
        Item::HideOthers => PredefinedMenuItem::hide_others(h, None),
        Item::ShowAll => PredefinedMenuItem::show_all(h, None),
        Item::Quit => PredefinedMenuItem::quit(h, None),
        Item::Undo => PredefinedMenuItem::undo(h, None),
        Item::Redo => PredefinedMenuItem::redo(h, None),
        Item::Cut => PredefinedMenuItem::cut(h, None),
        Item::Copy => PredefinedMenuItem::copy(h, None),
        Item::Paste => PredefinedMenuItem::paste(h, None),
        Item::SelectAll => PredefinedMenuItem::select_all(h, None),
        Item::Fullscreen => PredefinedMenuItem::fullscreen(h, None),
        Item::Minimize => PredefinedMenuItem::minimize(h, None),
        Item::Zoom => PredefinedMenuItem::maximize(h, None),
        Item::CloseWindow => PredefinedMenuItem::close_window(h, None),
        Item::Separator => PredefinedMenuItem::separator(h),
    }
}

/// Builds [`MENU`] (passed to `tauri::Builder::menu` on macOS).
///
/// # Errors
/// When a native menu item can't be made.
pub fn build<R: Runtime>(h: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let menu = Menu::new(h)?;
    for section in MENU {
        let sub = Submenu::new(h, section.title, true)?;
        for item in section.items {
            sub.append(&native(h, *item)?)?;
        }
        menu.append(&sub)?;
    }
    Ok(menu)
}
