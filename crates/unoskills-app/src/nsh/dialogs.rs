use std::path::PathBuf;

use nsh_core::{
    file_dialog::{FileFilter, OpenDialog, SaveDialog},
    prelude::*,
};

/// Desktop dialogs backed by the platform's file manager.
#[module]
pub struct Files;

#[module]
impl Files {
    /// Choose one directory or cancel.
    pub async fn folder(title: Option<String>) -> Result<Option<String>, String> {
        open(OpenDialog::folders(), title, Vec::new()).await
    }

    /// Choose one file or cancel.
    pub async fn open(
        title: Option<String>,
        extensions: Vec<String>,
    ) -> Result<Option<String>, String> {
        open(OpenDialog::files(), title, extensions).await
    }

    /// Choose an export destination or cancel.
    pub async fn save(
        title: Option<String>,
        name: Option<String>,
        extensions: Vec<String>,
    ) -> Result<Option<String>, String> {
        let mut dialog = SaveDialog::new();
        if let Some(title) = title {
            dialog = dialog.title(title);
        }
        if let Some(name) = name {
            dialog = dialog.file_name(name);
        }
        if !extensions.is_empty() {
            dialog = dialog.filter(extension_filter(extensions));
        }
        pick_save_path(dialog).await?.map(path_string).transpose()
    }

    /// Reveal a path in the desktop's file manager.
    pub async fn reveal(path: String) -> Result<(), String> {
        reveal_path(PathBuf::from(path)).await
    }
}

pub(super) fn registry() -> Registry {
    let mut registry = Registry::ui_set("unoskills/desktop");
    registry.module::<Files>();
    registry
}

async fn open(
    mut dialog: OpenDialog,
    title: Option<String>,
    extensions: Vec<String>,
) -> Result<Option<String>, String> {
    if let Some(title) = title {
        dialog = dialog.title(title);
    }
    if !extensions.is_empty() {
        dialog = dialog.filter(extension_filter(extensions));
    }
    pick_paths(dialog)
        .await?
        .and_then(|paths| paths.into_iter().next())
        .map(path_string)
        .transpose()
}

fn extension_filter(extensions: Vec<String>) -> FileFilter {
    FileFilter::new(extensions.join(", "), extensions)
}

fn path_string(path: PathBuf) -> Result<String, String> {
    path.into_os_string()
        .into_string()
        .map_err(|_| "INVALID_PATH_ENCODING".to_owned())
}
