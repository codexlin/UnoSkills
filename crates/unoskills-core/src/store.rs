use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::domain::{
    AgentCategory, DeployMode, Deployment, ProjectDeployment, ProjectSkillLock, ProjectWorkspace,
    Skill, SourceType, StackManifest,
};
use crate::registry::CustomAgentRecord;

pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut conn = Connection::open(db_path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;",
        )?;
        migrate(&mut conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn upsert_skill(&self, skill: &Skill) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let transaction = conn.transaction()?;
        transaction.execute(
            "INSERT INTO skills (
                id, slug, name, description, source_type, source_ref, source_subpath,
                source_branch, central_path, content_hash, estimated_tokens, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT(id) DO UPDATE SET
                slug = excluded.slug,
                name = excluded.name,
                description = excluded.description,
                source_type = excluded.source_type,
                source_ref = excluded.source_ref,
                source_subpath = excluded.source_subpath,
                source_branch = excluded.source_branch,
                central_path = excluded.central_path,
                content_hash = excluded.content_hash,
                estimated_tokens = excluded.estimated_tokens,
                updated_at = excluded.updated_at",
            params![
                skill.id,
                skill.slug,
                skill.name,
                skill.description,
                skill.source_type.as_str(),
                skill.source_ref,
                skill.source_subpath,
                skill.source_branch,
                skill.central_path.to_string_lossy().to_string(),
                skill.content_hash,
                skill.estimated_tokens as i64,
                skill.created_at,
                skill.updated_at,
            ],
        )?;

        transaction.execute(
            "DELETE FROM skill_tags WHERE skill_id = ?1",
            params![skill.id],
        )?;
        for tag in &skill.tags {
            transaction.execute(
                "INSERT OR IGNORE INTO skill_tags (skill_id, tag) VALUES (?1, ?2)",
                params![skill.id, tag.trim().to_lowercase()],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn list_skills(&self) -> Result<Vec<Skill>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, slug, name, description, source_type, source_ref, source_subpath,
                    source_branch, central_path, content_hash, estimated_tokens, created_at, updated_at
             FROM skills ORDER BY LOWER(name) ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Skill {
                id: row.get(0)?,
                slug: row.get(1)?,
                name: row.get(2)?,
                description: row.get(3)?,
                source_type: SourceType::from_str_lossy(&row.get::<_, String>(4)?),
                source_ref: row.get(5)?,
                source_subpath: row.get(6)?,
                source_branch: row.get(7)?,
                central_path: PathBuf::from(row.get::<_, String>(8)?),
                content_hash: row.get(9)?,
                estimated_tokens: row.get::<_, i64>(10)?.max(0) as u32,
                tags: Vec::new(),
                deployments: Vec::new(),
                project_deployments: Vec::new(),
                project_locks: Vec::new(),
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        })?;

        let mut skills: Vec<Skill> = rows.collect::<std::result::Result<_, _>>()?;
        let mut tags_map = load_all_tags(&conn)?;
        let mut dep_map = load_all_deployments(&conn)?;
        let mut proj_dep_map = load_all_project_deployments(&conn)?;
        let mut proj_lock_map = load_project_locks(&conn)?;

        for skill in &mut skills {
            skill.tags = tags_map.remove(&skill.id).unwrap_or_default();
            skill.deployments = dep_map.remove(&skill.id).unwrap_or_default();
            skill.project_deployments = proj_dep_map.remove(&skill.id).unwrap_or_default();
            skill.project_locks = proj_lock_map.remove(&skill.id).unwrap_or_default();
        }
        Ok(skills)
    }

    pub fn find_skill(&self, id_or_slug_or_name: &str) -> Result<Option<Skill>> {
        let input = id_or_slug_or_name.trim();
        let query = input.to_lowercase();
        let all = self.list_skills()?;
        if let Some(index) = all
            .iter()
            .position(|skill| skill.id == input)
            .or_else(|| all.iter().position(|skill| skill.slug == input))
            .or_else(|| {
                all.iter()
                    .position(|skill| skill.id.to_lowercase() == query)
            })
        {
            return Ok(all.into_iter().nth(index));
        }
        let (slugs, remaining): (Vec<_>, Vec<_>) = all
            .into_iter()
            .partition(|skill| skill.slug.to_lowercase() == query);
        let mut matches = if slugs.is_empty() {
            remaining
                .into_iter()
                .filter(|skill| skill.name.to_lowercase() == query)
                .collect()
        } else {
            slugs
        };
        if matches.len() > 1 {
            matches.sort_by(|a, b| a.slug.cmp(&b.slug));
            let choices = matches
                .iter()
                .map(|skill| format!("{} ({})", skill.slug, skill.id))
                .collect::<Vec<_>>()
                .join(", ");
            anyhow::bail!(
                "AMBIGUOUS_SKILL: {input:?} matches multiple skills. Use a skill ID from {choices}"
            );
        }
        Ok(matches.pop())
    }

    pub fn delete_skill(&self, skill_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM skills WHERE id = ?1", params![skill_id])?;
        Ok(())
    }

    pub fn upsert_deployment(&self, dep: &Deployment) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO deployments (id, skill_id, agent_key, target_path, mode, synced_at, copy_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(skill_id, agent_key) DO UPDATE SET
                target_path = excluded.target_path,
                mode = excluded.mode,
                synced_at = excluded.synced_at,
                copy_hash = excluded.copy_hash",
            params![
                dep.id,
                dep.skill_id,
                dep.agent_key,
                dep.target_path.to_string_lossy().to_string(),
                dep.mode.as_str(),
                dep.synced_at,
                dep.copy_hash,
            ],
        )?;
        Ok(())
    }

    pub fn delete_deployment(&self, skill_id: &str, agent_key: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM deployments WHERE skill_id = ?1 AND agent_key = ?2",
            params![skill_id, agent_key],
        )?;
        Ok(())
    }

    pub fn count_deployments_at_path(
        &self,
        target_path: &Path,
        exclude_deployment_id: &str,
    ) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let target_str = target_path.to_string_lossy().to_string();
        let count: i64 = conn.query_row(
            "SELECT
                 (SELECT COUNT(*) FROM deployments WHERE target_path = ?1 AND id != ?2) +
                 (SELECT COUNT(*) FROM project_deployments WHERE target_path = ?1 AND id != ?2)",
            params![target_str, exclude_deployment_id],
            |row| row.get(0),
        )?;
        Ok(count.max(0) as usize)
    }

    pub fn peer_deployment_mode_for_skill(
        &self,
        skill_id: &str,
        target_path: &Path,
        exclude_agent_key: &str,
    ) -> Result<Option<DeployMode>> {
        let conn = self.conn.lock().unwrap();
        let target_str = target_path.to_string_lossy().to_string();
        let mode_str = conn
            .query_row(
                "SELECT mode FROM deployments
                 WHERE skill_id = ?1 AND target_path = ?2 AND agent_key != ?3
                 LIMIT 1",
                params![skill_id, target_str, exclude_agent_key],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(mode_str.map(|s| DeployMode::from_str_lossy(&s)))
    }

    pub fn list_local_stacks(&self) -> Result<Vec<StackManifest>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT manifest_json FROM local_stacks ORDER BY updated_at DESC")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut stacks = Vec::new();
        for json in rows {
            if let Ok(manifest) = serde_json::from_str::<StackManifest>(&json?) {
                stacks.push(manifest);
            }
        }
        Ok(stacks)
    }

    pub(crate) fn find_local_stack(&self, id: &str) -> Result<Option<StackManifest>> {
        let conn = self.conn.lock().unwrap();
        let raw: Option<String> = conn
            .query_row(
                "SELECT manifest_json FROM local_stacks WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()?;
        raw.map(|raw| serde_json::from_str(&raw).context("Invalid saved stack manifest"))
            .transpose()
    }

    pub fn save_local_stack(&self, stack: &StackManifest, now: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let json = serde_json::to_string(stack)?;
        conn.execute(
            "INSERT INTO local_stacks (id, name, manifest_json, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                manifest_json = excluded.manifest_json,
                updated_at = excluded.updated_at",
            params![stack.id, stack.name, json, now],
        )?;
        Ok(())
    }

    pub fn delete_local_stack(&self, stack_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM local_stacks WHERE id = ?1", params![stack_id])?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let val = conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(val)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub(crate) fn get_backup_target(&self) -> Result<(Option<String>, Option<String>)> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row(
            "SELECT
                 (SELECT value FROM settings WHERE key = 'git_backup_repo_url'),
                 (SELECT value FROM settings WHERE key = 'git_backup_branch')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?)
    }

    pub(crate) fn set_backup_target(&self, repo_url: &str, branch: &str) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let transaction = conn.transaction()?;
        transaction.execute(
            "INSERT INTO settings (key, value) VALUES ('git_backup_repo_url', ?1), ('git_backup_branch', ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![repo_url, branch],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn get_agent_overrides(&self) -> Result<HashMap<String, String>> {
        let raw = self
            .get_setting("agent_path_overrides")?
            .unwrap_or_default();
        if raw.is_empty() {
            return Ok(HashMap::new());
        }
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    pub fn set_agent_override(&self, agent_key: &str, path: Option<&str>) -> Result<()> {
        let mut map = self.get_agent_overrides()?;
        match path {
            Some(p) if !p.trim().is_empty() => {
                map.insert(agent_key.to_string(), p.trim().to_string());
            }
            _ => {
                map.remove(agent_key);
            }
        }
        self.set_setting("agent_path_overrides", &serde_json::to_string(&map)?)
    }

    pub fn get_disabled_agents(&self) -> Result<Vec<String>> {
        let raw = self.get_setting("disabled_agents")?.unwrap_or_default();
        if raw.is_empty() {
            return Ok(Vec::new());
        }
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    pub fn set_agent_enabled(&self, agent_key: &str, enabled: bool) -> Result<()> {
        let mut disabled = self.get_disabled_agents()?;
        disabled.retain(|k| k != agent_key);
        if !enabled {
            disabled.push(agent_key.to_string());
        }
        self.set_setting("disabled_agents", &serde_json::to_string(&disabled)?)
    }

    pub fn list_custom_agents(&self) -> Result<Vec<CustomAgentRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT key, display_name, skills_dir, category FROM custom_agents ORDER BY key",
        )?;
        let rows = stmt.query_map([], |row| {
            let cat_str: String = row.get(3)?;
            let category = if cat_str == "assistant" {
                AgentCategory::Assistant
            } else {
                AgentCategory::Coding
            };
            Ok(CustomAgentRecord {
                key: row.get(0)?,
                display_name: row.get(1)?,
                skills_dir: row.get(2)?,
                category,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn upsert_custom_agent(&self, rec: &CustomAgentRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let cat = match rec.category {
            AgentCategory::Coding => "coding",
            AgentCategory::Assistant => "assistant",
        };
        conn.execute(
            "INSERT INTO custom_agents (key, display_name, skills_dir, category)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(key) DO UPDATE SET
                display_name = excluded.display_name,
                skills_dir = excluded.skills_dir,
                category = excluded.category",
            params![rec.key, rec.display_name, rec.skills_dir, cat],
        )?;
        Ok(())
    }

    pub fn delete_custom_agent(&self, key: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM custom_agents WHERE key = ?1", params![key])?;
        Ok(())
    }

    pub fn list_projects(&self) -> Result<Vec<ProjectWorkspace>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, name, path, created_at FROM projects ORDER BY LOWER(name) ASC")?;
        let rows = stmt.query_map([], |row| {
            Ok(ProjectWorkspace {
                id: row.get(0)?,
                name: row.get(1)?,
                path: PathBuf::from(row.get::<_, String>(2)?),
                created_at: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn find_project(&self, id_or_name_or_path: &str) -> Result<Option<ProjectWorkspace>> {
        let query = id_or_name_or_path.trim().to_lowercase();
        let all = self.list_projects()?;
        Ok(all.into_iter().find(|p| {
            p.id.to_lowercase() == query
                || p.name.to_lowercase() == query
                || p.path.to_string_lossy().to_lowercase() == query
        }))
    }

    pub fn upsert_project(&self, project: &ProjectWorkspace) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO projects (id, name, path, created_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(path) DO UPDATE SET
                name = excluded.name",
            params![
                project.id,
                project.name,
                project.path.to_string_lossy().to_string(),
                project.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn delete_project(&self, project_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM projects WHERE id = ?1", params![project_id])?;
        Ok(())
    }

    pub fn upsert_project_deployment(&self, dep: &ProjectDeployment) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO project_deployments (id, skill_id, project_id, agent_key, target_path, mode, synced_at, copy_hash, scope_path, follow_mode)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(skill_id, project_id, agent_key, scope_path) DO UPDATE SET
                target_path = excluded.target_path,
                mode = excluded.mode,
                synced_at = excluded.synced_at,
                copy_hash = excluded.copy_hash,
                follow_mode = excluded.follow_mode",
            params![
                dep.id,
                dep.skill_id,
                dep.project_id,
                dep.agent_key,
                dep.target_path.to_string_lossy().to_string(),
                dep.mode.as_str(),
                dep.synced_at,
                dep.copy_hash,
                dep.scope_path.to_string_lossy(),
                dep.follow_mode.map(|mode| mode.as_str()),
            ],
        )?;
        Ok(())
    }

    pub(crate) fn record_refreshed_copy(
        &self,
        skill_id: &str,
        target_path: &Path,
        copy_hash: Option<&str>,
    ) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let transaction = conn.transaction()?;
        transaction.execute(
            "UPDATE deployments SET copy_hash = ?1
             WHERE skill_id = ?2 AND target_path = ?3 AND mode = 'copy'",
            params![copy_hash, skill_id, target_path.to_string_lossy()],
        )?;
        transaction.execute(
            "UPDATE project_deployments SET copy_hash = ?1
             WHERE skill_id = ?2 AND target_path = ?3 AND mode = 'copy' AND NOT EXISTS (
                 SELECT 1 FROM project_skill_locks l WHERE l.skill_id = project_deployments.skill_id
                   AND l.project_id = project_deployments.project_id AND l.scope_path = project_deployments.scope_path
             )",
            params![copy_hash, skill_id, target_path.to_string_lossy()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn delete_project_deployment(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM project_deployments WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub(crate) fn update_project_versions(
        &self,
        deployments: &[ProjectDeployment],
        locked_hash: Option<&str>,
    ) -> Result<()> {
        let first = deployments.first().context("Missing project deployments")?;
        let mut conn = self.conn.lock().unwrap();
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for dep in deployments {
            let updated = transaction.execute(
                "UPDATE project_deployments SET mode = ?1, copy_hash = ?2, synced_at = ?3,
                 follow_mode = ?4 WHERE id = ?5 AND target_path = ?6",
                params![
                    dep.mode.as_str(),
                    dep.copy_hash,
                    dep.synced_at,
                    dep.follow_mode.map(|mode| mode.as_str()),
                    dep.id,
                    dep.target_path.to_string_lossy()
                ],
            )?;
            anyhow::ensure!(
                updated == 1,
                "STALE_PROJECT_VERSION: A deployment record changed"
            );
        }
        if let Some(hash) = locked_hash {
            transaction.execute(
                "INSERT INTO project_skill_locks (project_id, scope_path, skill_id, version_hash)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(project_id, scope_path, skill_id) DO UPDATE SET version_hash = excluded.version_hash",
                params![first.project_id, first.scope_path.to_string_lossy(), first.skill_id, hash],
            )?;
        } else {
            transaction.execute(
                "DELETE FROM project_skill_locks WHERE project_id = ?1 AND scope_path = ?2 AND skill_id = ?3",
                params![first.project_id, first.scope_path.to_string_lossy(), first.skill_id],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn locked_project_target_paths(&self) -> Result<Vec<PathBuf>> {
        let conn = self.conn.lock().unwrap();
        let mut statement = conn.prepare(
            "SELECT DISTINCT p.target_path FROM project_deployments p JOIN project_skill_locks l
             USING (skill_id, project_id, scope_path)",
        )?;
        let paths = statement.query_map([], |row| row.get::<_, String>(0).map(PathBuf::from))?;
        Ok(paths.collect::<std::result::Result<_, _>>()?)
    }

    pub(crate) fn project_snapshot_is_referenced(&self, hash: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM project_skill_locks WHERE version_hash = ?1)",
            [hash],
            |row| row.get(0),
        )?)
    }

    pub(crate) fn clear_retained_project_pin(
        &self,
        project_id: &str,
        scope: &Path,
        skill_id: &str,
        hash: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let removed = conn.execute(
            "DELETE FROM project_skill_locks
             WHERE project_id = ?1 AND scope_path = ?2 AND skill_id = ?3 AND version_hash = ?4
               AND NOT EXISTS (
                   SELECT 1 FROM project_deployments
                   WHERE project_id = ?1 AND scope_path = ?2 AND skill_id = ?3
               )",
            params![project_id, scope.to_string_lossy(), skill_id, hash],
        )?;
        anyhow::ensure!(
            removed == 1,
            "STALE_PROJECT_VERSION: The pin or its deployments changed"
        );
        Ok(())
    }

    pub fn list_project_deployments_for_project(
        &self,
        project_id: &str,
    ) -> Result<Vec<ProjectDeployment>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, skill_id, project_id, agent_key, target_path, mode, synced_at, copy_hash, scope_path, version_hash, follow_mode
             FROM project_deployments LEFT JOIN project_skill_locks USING (skill_id, project_id, scope_path)
             WHERE project_id = ?1 ORDER BY agent_key ASC",
        )?;
        let rows = stmt.query_map(params![project_id], project_deployment_from_row)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
}

fn load_all_tags(conn: &Connection) -> Result<HashMap<String, Vec<String>>> {
    let mut stmt = conn.prepare("SELECT skill_id, tag FROM skill_tags ORDER BY tag ASC")?;
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (skill_id, tag) = row?;
        map.entry(skill_id).or_default().push(tag);
    }
    Ok(map)
}

fn load_all_deployments(conn: &Connection) -> Result<HashMap<String, Vec<Deployment>>> {
    let mut stmt = conn.prepare(
        "SELECT id, skill_id, agent_key, target_path, mode, synced_at, copy_hash
         FROM deployments ORDER BY agent_key ASC",
    )?;
    let mut map: HashMap<String, Vec<Deployment>> = HashMap::new();
    let rows = stmt.query_map([], |row| {
        Ok(Deployment {
            id: row.get(0)?,
            skill_id: row.get(1)?,
            agent_key: row.get(2)?,
            target_path: PathBuf::from(row.get::<_, String>(3)?),
            mode: DeployMode::from_str_lossy(&row.get::<_, String>(4)?),
            synced_at: row.get(5)?,
            copy_hash: row.get(6)?,
        })
    })?;
    for row in rows {
        let dep = row?;
        map.entry(dep.skill_id.clone()).or_default().push(dep);
    }
    Ok(map)
}

fn load_all_project_deployments(
    conn: &Connection,
) -> Result<HashMap<String, Vec<ProjectDeployment>>> {
    let mut stmt = conn.prepare(
        "SELECT id, skill_id, project_id, agent_key, target_path, mode, synced_at, copy_hash, scope_path, version_hash, follow_mode
         FROM project_deployments LEFT JOIN project_skill_locks USING (skill_id, project_id, scope_path)
         ORDER BY project_id ASC, agent_key ASC",
    )?;
    let mut map: HashMap<String, Vec<ProjectDeployment>> = HashMap::new();
    let rows = stmt.query_map([], project_deployment_from_row)?;
    for row in rows {
        let dep = row?;
        map.entry(dep.skill_id.clone()).or_default().push(dep);
    }
    Ok(map)
}

fn load_project_locks(conn: &Connection) -> Result<HashMap<String, Vec<ProjectSkillLock>>> {
    let mut statement = conn.prepare(
        "SELECT skill_id, project_id, scope_path, version_hash FROM project_skill_locks",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            ProjectSkillLock {
                project_id: row.get(1)?,
                scope_path: PathBuf::from(row.get::<_, String>(2)?),
                hash: row.get(3)?,
            },
        ))
    })?;
    let mut locks = HashMap::<String, Vec<ProjectSkillLock>>::new();
    for row in rows {
        let (skill_id, lock) = row?;
        locks.entry(skill_id).or_default().push(lock);
    }
    Ok(locks)
}

fn project_deployment_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectDeployment> {
    Ok(ProjectDeployment {
        id: row.get(0)?,
        skill_id: row.get(1)?,
        project_id: row.get(2)?,
        agent_key: row.get(3)?,
        target_path: PathBuf::from(row.get::<_, String>(4)?),
        mode: DeployMode::from_str_lossy(&row.get::<_, String>(5)?),
        synced_at: row.get(6)?,
        copy_hash: row.get(7)?,
        scope_path: PathBuf::from(row.get::<_, String>(8)?),
        locked_hash: row.get(9)?,
        follow_mode: row
            .get::<_, Option<String>>(10)?
            .as_deref()
            .map(DeployMode::from_str_lossy),
    })
}

fn migrate(conn: &mut Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS skills (
            id TEXT PRIMARY KEY,
            slug TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            description TEXT,
            source_type TEXT NOT NULL,
            source_ref TEXT,
            source_subpath TEXT,
            source_branch TEXT,
            central_path TEXT NOT NULL,
            content_hash TEXT,
            estimated_tokens INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS skill_tags (
            skill_id TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
            tag TEXT NOT NULL,
            PRIMARY KEY (skill_id, tag)
        );

        CREATE TABLE IF NOT EXISTS deployments (
            id TEXT PRIMARY KEY,
            skill_id TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
            agent_key TEXT NOT NULL,
            target_path TEXT NOT NULL,
            mode TEXT NOT NULL,
            synced_at INTEGER NOT NULL,
            copy_hash TEXT,
            UNIQUE(skill_id, agent_key)
        );

        CREATE TABLE IF NOT EXISTS local_stacks (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            manifest_json TEXT NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS custom_agents (
            key TEXT PRIMARY KEY,
            display_name TEXT NOT NULL,
            skills_dir TEXT NOT NULL,
            category TEXT NOT NULL DEFAULT 'coding'
        );

        CREATE TABLE IF NOT EXISTS projects (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            path TEXT NOT NULL UNIQUE,
            created_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS project_deployments (
            id TEXT PRIMARY KEY,
            skill_id TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            agent_key TEXT NOT NULL,
            target_path TEXT NOT NULL,
            mode TEXT NOT NULL,
            synced_at INTEGER NOT NULL,
            copy_hash TEXT,
            scope_path TEXT NOT NULL DEFAULT '',
            UNIQUE(skill_id, project_id, agent_key, scope_path)
        );",
    )?;
    let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let has_global_copy_hash: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('deployments') WHERE name = 'copy_hash')",
        [],
        |row| row.get(0),
    )?;
    if !has_global_copy_hash {
        transaction.execute("ALTER TABLE deployments ADD COLUMN copy_hash TEXT", [])?;
    }
    let has_copy_hash: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('project_deployments') WHERE name = 'copy_hash')",
        [],
        |row| row.get(0),
    )?;
    if !has_copy_hash {
        transaction.execute(
            "ALTER TABLE project_deployments ADD COLUMN copy_hash TEXT",
            [],
        )?;
    }
    let has_scope: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('project_deployments') WHERE name = 'scope_path')",
        [],
        |row| row.get(0),
    )?;
    if !has_scope {
        transaction.execute_batch(
            "CREATE TABLE scoped_project_deployments (
                id TEXT PRIMARY KEY,
                skill_id TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
                project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                agent_key TEXT NOT NULL,
                target_path TEXT NOT NULL,
                mode TEXT NOT NULL,
                synced_at INTEGER NOT NULL,
                copy_hash TEXT,
                scope_path TEXT NOT NULL DEFAULT '',
                UNIQUE(skill_id, project_id, agent_key, scope_path)
            );
            INSERT INTO scoped_project_deployments
                (id, skill_id, project_id, agent_key, target_path, mode, synced_at, copy_hash)
                SELECT id, skill_id, project_id, agent_key, target_path, mode, synced_at, copy_hash
                FROM project_deployments;
            DROP TABLE project_deployments;
            ALTER TABLE scoped_project_deployments RENAME TO project_deployments;",
        )?;
    }
    let has_follow_mode: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('project_deployments') WHERE name = 'follow_mode')",
        [],
        |row| row.get(0),
    )?;
    if !has_follow_mode {
        transaction.execute(
            "ALTER TABLE project_deployments ADD COLUMN follow_mode TEXT",
            [],
        )?;
    }
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS project_skill_locks (
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            scope_path TEXT NOT NULL,
            skill_id TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
            version_hash TEXT NOT NULL,
            PRIMARY KEY(project_id, scope_path, skill_id)
        );",
    )?;
    #[cfg(windows)]
    transaction.execute_batch(
        "UPDATE project_deployments SET scope_path = REPLACE(scope_path, char(92), '/')
         WHERE INSTR(scope_path, char(92)) > 0;
         UPDATE project_skill_locks SET scope_path = REPLACE(scope_path, char(92), '/')
         WHERE INSTR(scope_path, char(92)) > 0;",
    )?;
    let has_locked_hash: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('project_deployments') WHERE name = 'locked_hash')",
        [],
        |row| row.get(0),
    )?;
    if has_locked_hash {
        let inconsistent: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM project_deployments
             GROUP BY project_id, scope_path, skill_id
             HAVING COUNT(locked_hash) > 0 AND
                (COUNT(locked_hash) != COUNT(*) OR COUNT(DISTINCT locked_hash) > 1))",
            [],
            |row| row.get(0),
        )?;
        anyhow::ensure!(!inconsistent, "Inconsistent legacy project version locks");
        transaction.execute_batch(
            "INSERT INTO project_skill_locks (project_id, scope_path, skill_id, version_hash)
             SELECT DISTINCT project_id, scope_path, skill_id, locked_hash
             FROM project_deployments WHERE locked_hash IS NOT NULL;
             ALTER TABLE project_deployments DROP COLUMN locked_hash;",
        )?;
    }
    transaction.commit()?;
    Ok(())
}
