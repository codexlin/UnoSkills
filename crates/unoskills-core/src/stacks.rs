use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::domain::StackManifest;

const FEATURED_STACKS_TOML: &str = include_str!("../data/featured_stacks.toml");

#[derive(Debug, Deserialize)]
struct FeaturedStacksFile {
    stacks: Vec<StackManifest>,
}

/// Wrapper for single-file `.unoskills-stack.toml` sharing / Gist / community PRs.
#[derive(Debug, Serialize, Deserialize)]
struct SingleStackFile {
    stack: StackManifest,
}

/// Abstraction over community Stack sources so UnoSkills can evolve cleanly from
/// Stage 1 (built-in + GitHub static `index.json`) to Stage 2 (Cloudflare Workers / Supabase Hub)
/// without changing UI or core orchestration code.
pub trait StackRegistry {
    fn fetch_featured(&self) -> Result<Vec<StackManifest>>;
}

#[derive(Default)]
pub struct DefaultStackRegistry {
    pub remote_index_url: Option<String>,
}

impl StackRegistry for DefaultStackRegistry {
    fn fetch_featured(&self) -> Result<Vec<StackManifest>> {
        let mut stacks = load_builtin_featured_stacks()?;
        if let Some(url) = &self.remote_index_url
            && let Ok(remote) = fetch_remote_index(url)
        {
            for r in remote {
                if !stacks.iter().any(|s| s.id == r.id) {
                    stacks.push(r);
                }
            }
        }
        Ok(stacks)
    }
}

pub fn load_builtin_featured_stacks() -> Result<Vec<StackManifest>> {
    let parsed: FeaturedStacksFile =
        toml::from_str(FEATURED_STACKS_TOML).context("Failed to parse built-in featured stacks")?;
    Ok(parsed.stacks)
}

fn fetch_remote_index(url: &str) -> Result<Vec<StackManifest>> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("unoskills/0.1")
        .timeout(std::time::Duration::from_secs(8))
        .build()?;
    let stacks: Vec<StackManifest> = client.get(url).send()?.json()?;
    Ok(stacks)
}

pub fn export_stack_to_toml(stack: &StackManifest) -> Result<String> {
    let wrapper = SingleStackFile {
        stack: stack.clone(),
    };
    toml::to_string_pretty(&wrapper).context("Failed to serialize stack manifest to TOML")
}

pub fn parse_stack_from_toml(toml_str: &str) -> Result<StackManifest> {
    if let Ok(wrapped) = toml::from_str::<SingleStackFile>(toml_str) {
        return Ok(wrapped.stack);
    }
    toml::from_str::<StackManifest>(toml_str).context("Invalid UnoSkills stack TOML manifest")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_featured_and_roundtrips_toml() {
        let featured = load_builtin_featured_stacks().unwrap();
        assert!(featured.len() >= 3);
        let toml_text = export_stack_to_toml(&featured[0]).unwrap();
        let parsed = parse_stack_from_toml(&toml_text).unwrap();
        assert_eq!(parsed.id, featured[0].id);
        assert_eq!(parsed.skills.len(), featured[0].skills.len());
    }
}
