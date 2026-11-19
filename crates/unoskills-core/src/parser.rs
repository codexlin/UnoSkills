use anyhow::Result;
use sha2::{Digest, Sha256};
use std::path::Path;
use walkdir::WalkDir;

use crate::domain::SkillLintReport;

/// Canonical and legacy-compatible skill entry files.
/// README.md and CLAUDE.md are intentionally NOT treated as skill markers.
pub const SKILL_MARKERS: &[&str] = &["SKILL.md", "skill.md"];

const WINDOWS_RESERVED_CHARS: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
const WINDOWS_RESERVED_BASENAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

#[derive(Debug, Clone, Default)]
pub struct ParsedSkillMeta {
    pub name: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub entry_file: Option<String>,
    pub raw_content: String,
    pub estimated_tokens: u32,
}

pub fn find_skill_entry(dir: &Path) -> Option<String> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut found_legacy = false;
    for entry in entries.flatten() {
        let is_file = entry.path().is_file();
        if !is_file {
            continue;
        }
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str == "SKILL.md" {
            return Some("SKILL.md".to_string());
        }
        if name_str == "skill.md" {
            found_legacy = true;
        }
    }
    if found_legacy {
        Some("skill.md".to_string())
    } else {
        None
    }
}

pub fn is_valid_skill_dir(dir: &Path) -> bool {
    dir.is_dir() && find_skill_entry(dir).is_some()
}

pub fn parse_skill_dir(dir: &Path) -> ParsedSkillMeta {
    let Some(entry_file) = find_skill_entry(dir) else {
        return ParsedSkillMeta::default();
    };
    let raw_content = std::fs::read_to_string(dir.join(&entry_file)).unwrap_or_default();
    let mut meta = parse_frontmatter(&raw_content);
    meta.entry_file = Some(entry_file);
    meta.estimated_tokens = estimate_tokens(&raw_content);
    meta.raw_content = raw_content;
    meta
}

pub fn parse_frontmatter(content: &str) -> ParsedSkillMeta {
    let trimmed = content.trim_start_matches('\u{feff}').trim_start();
    if !trimmed.starts_with("---") {
        return ParsedSkillMeta {
            estimated_tokens: estimate_tokens(content),
            ..Default::default()
        };
    }

    let rest = &trimmed[3..];
    let rest = rest
        .strip_prefix("\r\n")
        .or_else(|| rest.strip_prefix('\n'))
        .unwrap_or(rest);

    let end_offset = rest.find("\n---").or_else(|| rest.find("\r\n---"));

    let Some(end) = end_offset else {
        return ParsedSkillMeta {
            estimated_tokens: estimate_tokens(content),
            ..Default::default()
        };
    };

    let yaml_str = &rest[..end];
    if let Ok(yaml) = serde_yaml::from_str::<serde_yaml::Value>(yaml_str) {
        let name = yaml
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let description = yaml
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let tags = yaml
            .get("tags")
            .and_then(|v| v.as_sequence())
            .map(|seq| {
                seq.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        return ParsedSkillMeta {
            name,
            description,
            tags,
            entry_file: None,
            raw_content: String::new(),
            estimated_tokens: estimate_tokens(content),
        };
    }

    ParsedSkillMeta {
        estimated_tokens: estimate_tokens(content),
        ..Default::default()
    }
}

/// Strip leading YAML frontmatter (`--- ... ---`) so Markdown renderers do not
/// misinterpret the closing `---` as a CommonMark Setext H2 heading.
pub fn strip_frontmatter(content: &str) -> &str {
    let trimmed = content.trim_start_matches('\u{feff}');
    let leading_ws = trimmed.len() - trimmed.trim_start().len();
    let after_ws = &trimmed[leading_ws..];
    if !after_ws.starts_with("---") {
        return content;
    }

    let after_open = &after_ws[3..];
    let body_start = if let Some(rest) = after_open.strip_prefix("\r\n") {
        rest
    } else if let Some(rest) = after_open.strip_prefix('\n') {
        rest
    } else {
        return content;
    };

    if let Some(end_idx) = body_start
        .find("\n---")
        .or_else(|| body_start.find("\r\n---"))
    {
        let after_delim = if body_start[end_idx..].starts_with("\r\n---") {
            &body_start[end_idx + 5..]
        } else {
            &body_start[end_idx + 4..]
        };
        let rest = after_delim
            .strip_prefix("\r\n")
            .or_else(|| after_delim.strip_prefix('\n'))
            .unwrap_or(after_delim);
        return rest.trim_start_matches(['\r', '\n']);
    }

    content
}

/// Fast, CJK-aware token estimation (~4 ASCII chars/token, ~1.5 CJK chars/token).
pub fn estimate_tokens(text: &str) -> u32 {
    if text.is_empty() {
        return 0;
    }
    let mut ascii_chars = 0usize;
    let mut non_ascii_chars = 0usize;
    for ch in text.chars() {
        if ch.is_ascii() {
            ascii_chars += 1;
        } else {
            non_ascii_chars += 1;
        }
    }
    let est = (ascii_chars as f64 / 4.0) + (non_ascii_chars as f64 / 1.5);
    est.ceil().max(1.0) as u32
}

pub fn lint_skill_dir(dir: &Path) -> SkillLintReport {
    let meta = parse_skill_dir(dir);
    let mut warnings = Vec::new();

    if meta.entry_file.is_none() {
        warnings.push("Missing SKILL.md entry file".to_string());
    } else if meta.entry_file.as_deref() == Some("skill.md") {
        warnings.push("Uses legacy lowercase skill.md; prefer canonical SKILL.md".to_string());
    }
    if meta.name.is_none() {
        warnings.push("Missing `name` in YAML frontmatter".to_string());
    }
    if meta.description.is_none() {
        warnings.push(
            "Missing `description` in YAML frontmatter (agents rely on this to trigger the skill)"
                .to_string(),
        );
    }
    if meta.estimated_tokens > 4000 {
        warnings.push(format!(
            "SKILL.md is large (~{} tokens); consider splitting reference docs into references/",
            meta.estimated_tokens
        ));
    }

    let file_count = WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".git")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .count();

    SkillLintReport {
        valid: meta.entry_file.is_some(),
        entry_file: meta.entry_file,
        estimated_tokens: meta.estimated_tokens,
        file_count,
        warnings,
    }
}

/// Sanitize a skill name into a safe single directory slug across macOS/Linux/Windows.
pub fn sanitize_slug(raw: &str) -> Option<String> {
    let last = Path::new(raw)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())?;
    if last == "." || last == ".." {
        return None;
    }

    let clean: String = last
        .chars()
        .map(|c| {
            if c.is_control() || WINDOWS_RESERVED_CHARS.contains(&c) {
                '-'
            } else {
                c
            }
        })
        .collect();

    let trimmed = clean.trim().trim_end_matches('.');
    if trimmed.is_empty() {
        return None;
    }

    let base_upper = trimmed
        .split('.')
        .next()
        .unwrap_or(trimmed)
        .to_ascii_uppercase();
    if WINDOWS_RESERVED_BASENAMES.contains(&base_upper.as_str()) {
        Some(format!("_{trimmed}"))
    } else {
        Some(trimmed.to_string())
    }
}

/// Compute a deterministic SHA-256 hash of a skill directory's files (excluding `.git`).
pub fn hash_directory(dir: &Path) -> Result<String> {
    let mut rel_files: Vec<(String, std::path::PathBuf)> = Vec::new();
    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".git")
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file()
            && let Ok(rel) = entry.path().strip_prefix(dir)
        {
            let norm = rel.to_string_lossy().replace('\\', "/");
            rel_files.push((norm, entry.into_path()));
        }
    }
    rel_files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = Sha256::new();
    for (rel, path) in rel_files {
        hasher.update(rel.as_bytes());
        hasher.update(b"\0");
        let bytes = std::fs::read(&path)?;
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
        hasher.update(b"\n");
    }
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter_and_tags() {
        let md = "---\nname: rust-gpui\ndescription: Build native GPUI apps\ntags:\n  - Rust\n  - UI\n---\n# Guide\nHello world";
        let meta = parse_frontmatter(md);
        assert_eq!(meta.name.as_deref(), Some("rust-gpui"));
        assert_eq!(meta.description.as_deref(), Some("Build native GPUI apps"));
        assert_eq!(meta.tags, vec!["rust", "ui"]);
        assert!(meta.estimated_tokens > 0);
    }

    #[test]
    fn rejects_readme_as_skill_marker() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("README.md"), "# Not a skill").unwrap();
        assert!(!is_valid_skill_dir(dir.path()));
        std::fs::write(dir.path().join("SKILL.md"), "---\nname: ok\n---\n").unwrap();
        assert!(is_valid_skill_dir(dir.path()));
    }

    #[test]
    fn strips_frontmatter_cleanly() {
        let md = "---\nname: demo\ndescription: \"Hi\"\n---\n\n# Real Heading\nBody";
        assert_eq!(strip_frontmatter(md), "# Real Heading\nBody");
        assert_eq!(strip_frontmatter("# Plain"), "# Plain");
    }

    #[test]
    fn sanitizes_traversal_and_reserved() {
        assert_eq!(sanitize_slug("../../evil"), Some("evil".to_string()));
        assert_eq!(sanitize_slug(".."), None);
        assert_eq!(sanitize_slug("CON"), Some("_CON".to_string()));
    }
}
