#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod nsh;
#[cfg(test)]
mod smoke;

#[cfg(target_os = "linux")]
use nsh_platform_linux as desktop;
#[cfg(target_os = "macos")]
use nsh_platform_macos as desktop;
#[cfg(target_os = "windows")]
use nsh_platform_windows as desktop;

use std::process::ExitCode;

use nsh_engine::Bundle;

const APP_ID: &str = "dev.unoskills.UnoSkills";

fn main() -> ExitCode {
    env_logger::init();
    match run() {
        Ok(exit) => exit,
        Err(error) => {
            eprintln!("UnoSkills: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn bundle() -> anyhow::Result<Bundle> {
    let config: serde_json::Value = serde_json::from_str(include_str!("../ui/nsh.json"))?;
    Bundle::source(include_str!(concat!(env!("OUT_DIR"), "/ui.js")).to_owned())
        .with_app_id(Some(APP_ID))
        .with_main_window(Some(&config["window"].to_string()))
        .with_capabilities_json(Some(&config["capabilities"].to_string()))
        .map_err(anyhow::Error::msg)
}

fn run() -> anyhow::Result<ExitCode> {
    let bundle = bundle()?;
    desktop::set_ui_sets(vec![nsh_base::registry(), nsh::dialogs::registry()]);
    let options = desktop::AppOptions {
        app_id: APP_ID.to_owned(),
        title: "UnoSkills".to_owned(),
        size: (1240, 780),
        host: nsh::channel()?,
        ..Default::default()
    };
    let exit = desktop::run_app(desktop::AppSource::Bundle(bundle), options);
    #[cfg(target_os = "linux")]
    let exit = ExitCode::from(exit);
    Ok(exit)
}
