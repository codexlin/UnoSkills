mod discover_ops;
mod library_ops;

use anyhow::{Context, Result, bail};
use serde_json::Value;
use unoskills_core::{DeployMode, UnoSkillsManager};

/// Dispatches all `library.*` and `discover.*` NSH host commands against the
/// shared [`UnoSkillsManager`] facade.
///
/// Transport framing and error-code mapping are owned by the desktop shell;
/// handlers return raw JSON payloads on success or propagate typed `anyhow`
/// errors (preserving core sentinels such as `TARGET_CONFLICT`).
pub(super) fn dispatch(manager: &UnoSkillsManager, command: &str, input: Value) -> Result<Value> {
    if let Some(sub) = command.strip_prefix("library.") {
        return library_ops::dispatch(manager, sub, input);
    }
    if let Some(sub) = command.strip_prefix("discover.") {
        return discover_ops::dispatch(manager, sub, input);
    }
    bail!("Unknown library/discover host command: '{command}'")
}

pub(super) fn required_str<'a>(input: &'a Value, field: &str) -> Result<&'a str> {
    input
        .get(field)
        .and_then(Value::as_str)
        .with_context(|| format!("Missing required string field '{field}'"))
}

pub(super) fn optional_str<'a>(input: &'a Value, field: &str) -> Result<Option<&'a str>> {
    match input.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.as_str())),
        Some(_) => bail!("Field '{field}' must be a string or null"),
    }
}

pub(super) fn optional_bool(input: &Value, field: &str) -> Result<bool> {
    match input.get(field) {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => bail!("Field '{field}' must be a boolean"),
    }
}

pub(super) fn optional_usize(input: &Value, field: &str, default: usize) -> Result<usize> {
    match input.get(field) {
        None | Some(Value::Null) => Ok(default),
        Some(value) => {
            let raw = value
                .as_u64()
                .with_context(|| format!("Field '{field}' must be a non-negative integer"))?;
            usize::try_from(raw).with_context(|| format!("Field '{field}' exceeds platform range"))
        }
    }
}

pub(super) fn optional_deploy_mode(input: &Value, field: &str) -> Result<Option<DeployMode>> {
    let Some(raw) = optional_str(input, field)? else {
        return Ok(None);
    };
    match raw.trim().to_ascii_lowercase().as_str() {
        "" => Ok(None),
        "symlink" => Ok(Some(DeployMode::Symlink)),
        "copy" => Ok(Some(DeployMode::Copy)),
        other => bail!("Invalid deploy mode '{other}'; expected 'symlink' or 'copy'"),
    }
}
