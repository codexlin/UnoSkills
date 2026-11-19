use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::domain::{
    AgentCategory, DeployMode, Deployment, ProjectDeployment, ProjectWorkspace, Skill, SourceType,
    StackManifest,
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
        let conn = Connection::open(db_path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;",
        )?;
        migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn upsert_skill(&self, skill: &Skill) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
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

        conn.execute(
            "DELETE FROM skill_tags WHERE skill_id = ?1",
            params![skill.id],
        )?;
        for tag in &skill.tags {
            conn.execute(
                "INSERT OR IGNORE INTO skill_tags (skill_id, tag) VALUES (?1, ?2)",
                params![skill.id, tag.trim().to_lowercase()],
            )?;
        }
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
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        })?;

        let mut skills: Vec<Skill> = rows.collect::<std::result::Result<_, _>>()?;
        let mut tags_map = load_all_tags(&conn)?;
        let mut dep_map = load_all_deployments(&conn)?;
        let mut proj_dep_map = load_all_project_deployments(&conn)?;

        for skill in &mut skills {
            skill.tags = tags_map.remove(&skill.id).unwrap_or_default();
            skill.deployments = dep_map.remove(&skill.id).unwrap_or_default();
            skill.project_deployments = proj_dep_map.remove(&skill.id).unwrap_or_default();
        }
        Ok(skills)
    }

    pub fn find_skill(&self, id_or_slug_or_name: &str) -> Result<Option<Skill>> {
        let query = id_or_slug_or_name.trim().to_lowercase();
        let all = self.list_skills()?;
        Ok(all.into_iter().find(|s| {
            s.id.to_lowercase() == query
                || s.slug.to_lowercase() == query
                || s.name.to_lowercase() == query
        }))
    }

    pub fn delete_skill(&self, skill_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM skills WHERE id = ?1", params![skill_id])?;
        Ok(())
    }

    pub fn upsert_deployment(&self, dep: &Deployment) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO deployments (id, skill_id, agent_key, target_path, mode, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(skill_id, agent_key) DO UPDATE SET
                target_path = excluded.target_path,
                mode = excluded.mode,
                synced_at = excluded.synced_at",
            params![
                dep.id,
                dep.skill_id,
                dep.agent_key,
                dep.target_path.to_string_lossy().to_string(),
                dep.mode.as_str(),
                dep.synced_at,
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
        exclude_agent_key: &str,
    ) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let target_str = target_path.to_string_lossy().to_string();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM deployments WHERE target_path = ?1 AND agent_key != ?2",
            params![target_str, exclude_agent_key],
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
            "INSERT INTO project_deployments (id, skill_id, project_id, agent_key, target_path, mode, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(skill_id, project_id, agent_key) DO UPDATE SET
                target_path = excluded.target_path,
                mode = excluded.mode,
                synced_at = excluded.synced_at",
            params![
                dep.id,
                dep.skill_id,
                dep.project_id,
                dep.agent_key,
                dep.target_path.to_string_lossy().to_string(),
                dep.mode.as_str(),
                dep.synced_at,
            ],
        )?;
        Ok(())
    }

    pub fn delete_project_deployment(
        &self,
        skill_id: &str,
        project_id: &str,
        agent_key: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM project_deployments WHERE skill_id = ?1 AND project_id = ?2 AND agent_key = ?3",
            params![skill_id, project_id, agent_key],
        )?;
        Ok(())
    }

    pub fn list_project_deployments_for_project(
        &self,
        project_id: &str,
    ) -> Result<Vec<ProjectDeployment>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, skill_id, project_id, agent_key, target_path, mode, synced_at
             FROM project_deployments WHERE project_id = ?1 ORDER BY agent_key ASC",
        )?;
        let rows = stmt.query_map(params![project_id], |row| {
            Ok(ProjectDeployment {
                id: row.get(0)?,
                skill_id: row.get(1)?,
                project_id: row.get(2)?,
                agent_key: row.get(3)?,
                target_path: PathBuf::from(row.get::<_, String>(4)?),
                mode: DeployMode::from_str_lossy(&row.get::<_, String>(5)?),
                synced_at: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn count_project_deployments_at_path(
        &self,
        project_id: &str,
        target_path: &Path,
        exclude_agent_key: &str,
    ) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let target_str = target_path.to_string_lossy().to_string();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM project_deployments
             WHERE project_id = ?1 AND target_path = ?2 AND agent_key != ?3",
            params![project_id, target_str, exclude_agent_key],
            |row| row.get(0),
        )?;
        Ok(count.max(0) as usize)
    }

    pub fn peer_project_deployment_mode_for_skill(
        &self,
        skill_id: &str,
        project_id: &str,
        target_path: &Path,
        exclude_agent_key: &str,
    ) -> Result<Option<DeployMode>> {
        let conn = self.conn.lock().unwrap();
        let target_str = target_path.to_string_lossy().to_string();
        let mode_str = conn
            .query_row(
                "SELECT mode FROM project_deployments
                 WHERE skill_id = ?1 AND project_id = ?2 AND target_path = ?3 AND agent_key != ?4
                 LIMIT 1",
                params![skill_id, project_id, target_str, exclude_agent_key],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(mode_str.map(|s| DeployMode::from_str_lossy(&s)))
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
        "SELECT id, skill_id, agent_key, target_path, mode, synced_at
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
        "SELECT id, skill_id, project_id, agent_key, target_path, mode, synced_at
         FROM project_deployments ORDER BY project_id ASC, agent_key ASC",
    )?;
    let mut map: HashMap<String, Vec<ProjectDeployment>> = HashMap::new();
    let rows = stmt.query_map([], |row| {
        Ok(ProjectDeployment {
            id: row.get(0)?,
            skill_id: row.get(1)?,
            project_id: row.get(2)?,
            agent_key: row.get(3)?,
            target_path: PathBuf::from(row.get::<_, String>(4)?),
            mode: DeployMode::from_str_lossy(&row.get::<_, String>(5)?),
            synced_at: row.get(6)?,
        })
    })?;
    for row in rows {
        let dep = row?;
        map.entry(dep.skill_id.clone()).or_default().push(dep);
    }
    Ok(map)
}

fn migrate(conn: &Connection) -> Result<()> {
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
            UNIQUE(skill_id, project_id, agent_key)
        );",
    )?;
    Ok(())
}
