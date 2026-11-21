use std::sync::{Mutex, MutexGuard};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use unoskills_core::{
    UnoSkillsManager,
    updater::{
        DownloadedUpdate, PlatformKind, UpdateCheckOutcome, UpdateProgress,
        launch_platform_installer,
    },
};

#[derive(Default)]
pub(super) struct Updates {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    checking: bool,
    outcome: Option<UpdateCheckOutcome>,
    download: Download,
}

#[derive(Default)]
enum Download {
    #[default]
    Idle,
    Progress(UpdateProgress),
    Ready(DownloadedUpdate),
    Failed(String),
}

impl Updates {
    pub(super) fn dispatch(&self, manager: &UnoSkillsManager, command: &str) -> Result<Value> {
        match command {
            "update.check" => self.check(manager),
            "update.download" => self.download(manager),
            "update.install" => self.install(),
            _ => bail!("UNKNOWN_COMMAND: {command}"),
        }
    }

    fn state(&self) -> Result<MutexGuard<'_, State>> {
        self.state
            .lock()
            .map_err(|_| anyhow::anyhow!("UPDATE_STATE_UNAVAILABLE"))
    }

    pub(super) fn status(&self) -> Result<Value> {
        let state = self.state()?;
        let download = match &state.download {
            Download::Idle => json!({"phase": "idle"}),
            Download::Progress(UpdateProgress::Preparing) => json!({"phase": "preparing"}),
            Download::Progress(UpdateProgress::Downloading(percent)) => {
                json!({"phase": "downloading", "percent": percent})
            }
            Download::Progress(UpdateProgress::Verifying) => json!({"phase": "verifying"}),
            Download::Ready(file) => json!({"phase": "ready", "verified": file.verified}),
            Download::Failed(error) => json!({"phase": "failed", "error": error}),
        };
        Ok(json!({
            "version": env!("CARGO_PKG_VERSION"),
            "platform": PlatformKind::current(),
            "checking": state.checking,
            "available": state.outcome.as_ref().filter(|outcome| outcome.update_available),
            "download": download,
        }))
    }

    fn check(&self, manager: &UnoSkillsManager) -> Result<Value> {
        {
            let mut state = self.state()?;
            if state.checking || matches!(state.download, Download::Progress(_)) {
                bail!("UPDATE_BUSY");
            }
            state.checking = true;
        }
        let result = manager.check_app_update(env!("CARGO_PKG_VERSION"));
        {
            let mut state = self.state()?;
            state.checking = false;
            let outcome = result?;
            if !outcome.update_available
                || state.outcome.as_ref().and_then(release_tag) != release_tag(&outcome)
            {
                state.download = Download::Idle;
            }
            state.outcome = Some(outcome);
        }
        self.status()
    }

    fn download(&self, manager: &UnoSkillsManager) -> Result<Value> {
        let (tag, asset, checksums) = {
            let mut state = self.state()?;
            if state.checking || !matches!(state.download, Download::Idle | Download::Failed(_)) {
                bail!("UPDATE_BUSY");
            }
            let outcome = state
                .outcome
                .as_ref()
                .filter(|outcome| outcome.update_available);
            let outcome = outcome.context("NO_UPDATE_AVAILABLE")?;
            let tag = release_tag(outcome)
                .context("NO_UPDATE_AVAILABLE")?
                .to_owned();
            let asset = outcome
                .recommended_asset
                .clone()
                .context("NO_INSTALLER_AVAILABLE")?;
            let checksums = outcome.checksums_asset.clone();
            state.download = Download::Progress(UpdateProgress::Preparing);
            (tag, asset, checksums)
        };
        let result = manager.download_app_update_with_progress(
            &tag,
            &asset,
            checksums.as_ref(),
            |progress| {
                if let Ok(mut state) = self.state() {
                    state.download = Download::Progress(progress);
                }
            },
        );
        self.state()?.download = match result {
            Ok(file) => Download::Ready(file),
            Err(error) => Download::Failed(format!("{error:#}")),
        };
        self.status()
    }

    fn install(&self) -> Result<Value> {
        let path = match &self.state()?.download {
            Download::Ready(file) => file.file_path.clone(),
            _ => bail!("UPDATE_NOT_DOWNLOADED"),
        };
        match launch_platform_installer(&path) {
            Ok(()) => Ok(json!({"launched": true})),
            Err(error) => {
                Ok(json!({"launched": false, "path": path, "error": format!("{error:#}")}))
            }
        }
    }
}

fn release_tag(outcome: &UpdateCheckOutcome) -> Option<&str> {
    outcome
        .latest
        .as_ref()
        .map(|release| release.tag_name.as_str())
}
