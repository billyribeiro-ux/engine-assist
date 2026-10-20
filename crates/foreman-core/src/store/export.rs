//! User exports (`WritePurpose::UserExport`, SR-30, FILE-R9): Export metrics and Export run sheet
//! write one `.md` file at a path Billy picked in the Rust save dialog. The only write outside
//! projects and app data. Same atomic rules as every other write: no symlink, no hard link, never
//! over a file that changed since it was read.

use std::path::Path;

use super::{Root, RootKind, StoreError};
use crate::pathpolicy::{self, ReadClass};

/// Writes `bytes` to `target`.
///
/// # Errors
/// [`StoreError::Policy`] when the target isn't an allowed export, others as [`Root::write`].
pub fn write_user_export(target: &Path, bytes: &[u8], app_data: &Path) -> Result<(), StoreError> {
    let name = target.display().to_string();
    let (Some(parent), Some(file)) = (target.parent(), target.file_name().and_then(|f| f.to_str()))
    else {
        return Err(StoreError::NotFound(name));
    };
    let dir = super::canonical(parent);
    let app_data = super::canonical(app_data);
    let rel = pathpolicy::check_user_export(&dir, file, &app_data)?;
    let root = Root::open(&dir, RootKind::Export)?;
    let (_, stamp) = root.read_opt(&rel, ReadClass::AppState)?;
    root.write(&rel, bytes, &stamp)?;
    Ok(())
}
