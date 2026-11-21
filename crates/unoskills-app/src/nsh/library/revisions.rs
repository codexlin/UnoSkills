use std::sync::Mutex;

use anyhow::{Context, Result};
use serde_json::Value;
use unoskills_core::{PreparedSkillRevision, UnoSkillsManager};

use super::required_str;

/// The desktop has one revision confirmation dialog. Keeping its prepared
/// files here lets the host apply the reviewed files without fetching again.
#[derive(Default)]
pub(in super::super) struct RevisionPreviews {
    pending: Mutex<Option<PreparedSkillRevision>>,
}

impl RevisionPreviews {
    pub(in super::super) fn dispatch(
        &self,
        manager: &UnoSkillsManager,
        command: &str,
        input: &Value,
    ) -> Option<Result<Value>> {
        Some(match command {
            "library.prepare_skill_update" => self.prepare(manager, input, false),
            "library.prepare_skill_rollback" => self.prepare(manager, input, true),
            "library.apply_skill_revision" => self.apply(manager, input),
            "library.cancel_skill_revision" => self.cancel(input),
            _ => return None,
        })
    }

    fn prepare(&self, manager: &UnoSkillsManager, input: &Value, rollback: bool) -> Result<Value> {
        let skill_id = required_str(input, "skill_id")?;
        let prepared = if rollback {
            manager.prepare_skill_rollback(skill_id)?
        } else {
            manager.prepare_skill_update(skill_id)?
        };
        let result = serde_json::to_value(prepared.as_ref().map(PreparedSkillRevision::preview))?;
        *self.pending.lock().unwrap() = prepared;
        Ok(result)
    }

    fn take(&self, id: &str) -> Option<PreparedSkillRevision> {
        let mut guard = self.pending.lock().unwrap();
        if guard.as_ref().is_some_and(|item| item.preview().id == id) {
            guard.take()
        } else {
            None
        }
    }

    fn apply(&self, manager: &UnoSkillsManager, input: &Value) -> Result<Value> {
        let prepared = self
            .take(required_str(input, "preview_id")?)
            .context("STALE_SKILL_REVISION: Check the skill again before applying.")?;
        Ok(serde_json::to_value(
            manager.apply_skill_revision(prepared)?,
        )?)
    }

    fn cancel(&self, input: &Value) -> Result<Value> {
        self.take(required_str(input, "preview_id")?);
        Ok(Value::Null)
    }
}
