use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::json;
use std::path::PathBuf;
use unoskills_core::{DeployMode, UnoSkillsManager};

#[derive(Parser)]
#[command(
    name = "unoskills",
    version,
    about = "Fast, native Rust CLI & workbench for AI agent skills & stacks"
)]
struct Cli {
    /// Custom UnoSkills root directory (default: ~/.unoskills)
    #[arg(long, global = true)]
    root: Option<PathBuf>,

    /// Emit machine-readable JSON output (ideal for AI agents & scripts)
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List installed skills in the central library
    List,
    /// Show detailed info and lint report for a skill
    Show { skill: String },
    /// Install a skill from local path, Git URL, or owner/repo@skill
    Install { source: String },
    /// Remove a skill from the library and undeploy from all agents
    Remove {
        skill: String,
        #[arg(long)]
        yes: bool,
    },
    /// Deploy a skill to one or more agents
    Deploy {
        skill: String,
        #[arg(long = "agent", required = true)]
        agents: Vec<String>,
        #[arg(long)]
        copy: bool,
        #[arg(long)]
        force: bool,
    },
    /// Undeploy a skill from one or more agents
    Undeploy {
        skill: String,
        #[arg(long = "agent", required = true)]
        agents: Vec<String>,
    },
    /// Scan installed agents for unmanaged skills and optionally adopt them
    Adopt {
        #[arg(long)]
        dry_run: bool,
    },
    /// List supported AI agents and detection status
    Agents,
    /// Manage and share Skill Stacks (curated & local combinations)
    Stack {
        #[command(subcommand)]
        action: StackCommands,
    },
}

#[derive(Subcommand)]
enum StackCommands {
    /// List featured and local Skill Stacks
    List,
    /// Install a featured, local, or TOML-file Skill Stack
    Install {
        /// Featured stack ID or path to a .toml stack manifest
        stack: String,
        /// Optionally deploy all skills in this stack to specified agents
        #[arg(long = "agent")]
        agents: Vec<String>,
    },
    /// Export a local stack (or featured stack) as shareable TOML
    Export { stack: String },
    /// Create a new local stack from installed skill slugs
    Create {
        name: String,
        #[arg(long, default_value = "local")]
        author: String,
        #[arg(long, default_value = "")]
        description: String,
        #[arg(long = "skill", required = true)]
        skills: Vec<String>,
    },
}

fn main() {
    let cli = Cli::parse();
    let json_mode = cli.json;
    if let Err(err) = run(cli) {
        if json_mode {
            eprintln!(
                "{}",
                json!({
                    "ok": false,
                    "error": err.to_string(),
                })
            );
        } else {
            eprintln!("error: {err}");
        }
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    let mgr = match cli.root {
        Some(root) => UnoSkillsManager::open_at(root)?,
        None => UnoSkillsManager::open_default()?,
    };

    match cli.command {
        Commands::List => {
            let skills = mgr.list_skills()?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&skills)?);
            } else if skills.is_empty() {
                println!(
                    "No skills installed yet. Try `unoskills install <source>` or `unoskills adopt`."
                );
            } else {
                for s in skills {
                    let agents: Vec<&str> =
                        s.deployments.iter().map(|d| d.agent_key.as_str()).collect();
                    println!(
                        "{:<28} ~{:>4} tok  agents=[{}]",
                        s.name,
                        s.estimated_tokens,
                        agents.join(", ")
                    );
                }
            }
        }
        Commands::Show { skill } => {
            let found = mgr
                .find_skill(&skill)?
                .ok_or_else(|| anyhow::anyhow!("Skill '{skill}' not found"))?;
            let lint = mgr.lint_skill(&found.id)?;
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "skill": found,
                        "lint": lint,
                    }))?
                );
            } else {
                println!("Name:        {}", found.name);
                println!("Slug:        {}", found.slug);
                println!("Path:        {}", found.central_path.display());
                println!(
                    "Source:      {:?} ({})",
                    found.source_type,
                    found.source_ref.as_deref().unwrap_or("-")
                );
                println!("Tokens:      ~{}", found.estimated_tokens);
                println!("Files:       {}", lint.file_count);
                if let Some(desc) = &found.description {
                    println!("Description: {desc}");
                }
                if !lint.warnings.is_empty() {
                    println!("Warnings:");
                    for w in &lint.warnings {
                        println!("  - {w}");
                    }
                }
            }
        }
        Commands::Install { source } => {
            let skill = mgr.install_from_ref(&source)?;
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({ "ok": true, "skill": skill }))?
                );
            } else {
                println!(
                    "Installed '{}' (~{} tokens) -> {}",
                    skill.name,
                    skill.estimated_tokens,
                    skill.central_path.display()
                );
            }
        }
        Commands::Remove { skill, yes } => {
            if !yes {
                anyhow::bail!("Refusing to remove '{skill}' without --yes");
            }
            mgr.remove_skill(&skill)?;
            if cli.json {
                println!("{}", json!({ "ok": true, "removed": skill }));
            } else {
                println!("Removed '{skill}'");
            }
        }
        Commands::Deploy {
            skill,
            agents,
            copy,
            force,
        } => {
            let mode = if copy { Some(DeployMode::Copy) } else { None };
            let mut results = Vec::new();
            for agent in &agents {
                let dep = mgr.deploy_skill(&skill, agent, mode, force)?;
                results.push(dep);
            }
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({ "ok": true, "deployments": results }))?
                );
            } else {
                for dep in results {
                    println!(
                        "Deployed to {} ({:?}) -> {}",
                        dep.agent_key,
                        dep.mode,
                        dep.target_path.display()
                    );
                }
            }
        }
        Commands::Undeploy { skill, agents } => {
            for agent in &agents {
                mgr.undeploy_skill(&skill, agent)?;
            }
            if cli.json {
                println!("{}", json!({ "ok": true, "undeployed": agents }));
            } else {
                println!("Undeployed '{skill}' from {}", agents.join(", "));
            }
        }
        Commands::Adopt { dry_run } => {
            let candidates = mgr.scan_adoptable()?;
            if dry_run {
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&candidates)?);
                } else if candidates.is_empty() {
                    println!("No unmanaged skills found in installed agent directories.");
                } else {
                    for c in &candidates {
                        println!("[{}] {} ({})", c.agent_name, c.name, c.path.display());
                    }
                }
            } else {
                let mut adopted = Vec::new();
                for c in &candidates {
                    if let Ok(skill) = mgr.adopt_candidate(c) {
                        adopted.push(skill);
                    }
                }
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&json!({ "ok": true, "adopted": adopted }))?
                    );
                } else {
                    println!("Adopted {} skill(s).", adopted.len());
                }
            }
        }
        Commands::Agents => {
            let agents = mgr.list_agents()?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&agents)?);
            } else {
                for a in agents {
                    let status = if a.installed {
                        "installed"
                    } else {
                        "not found"
                    };
                    println!(
                        "{:<16} {:<16} {:<10} {}",
                        a.key,
                        a.display_name,
                        status,
                        a.skills_dir.display()
                    );
                }
            }
        }
        Commands::Stack { action } => match action {
            StackCommands::List => {
                let featured = mgr.featured_stacks()?;
                let local = mgr.list_local_stacks()?;
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &json!({ "featured": featured, "local": local })
                        )?
                    );
                } else {
                    println!("=== Featured Stacks ===");
                    for s in featured {
                        println!("  {:<26} ({} skills) — {}", s.id, s.skills.len(), s.name);
                    }
                    println!("\n=== My Local Stacks ===");
                    if local.is_empty() {
                        println!("  (none yet — create one with `unoskills stack create`)");
                    } else {
                        for s in local {
                            println!("  {:<26} ({} skills) — {}", s.id, s.skills.len(), s.name);
                        }
                    }
                }
            }
            StackCommands::Install { stack, agents } => {
                let manifest = if PathBuf::from(&stack).is_file() {
                    let text = std::fs::read_to_string(&stack)?;
                    mgr.import_stack_toml(&text)?
                } else {
                    let all = [mgr.list_local_stacks()?, mgr.featured_stacks()?].concat();
                    all.into_iter()
                        .find(|s| s.id == stack || s.name.eq_ignore_ascii_case(&stack))
                        .ok_or_else(|| anyhow::anyhow!("Stack '{stack}' not found"))?
                };
                let summary = mgr.install_stack(&manifest, &agents)?;
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&summary)?);
                } else {
                    println!(
                        "Stack '{}': installed {}, skipped existing {}, failed {}",
                        summary.stack_id,
                        summary.installed.len(),
                        summary.skipped_existing.len(),
                        summary.failed.len()
                    );
                }
            }
            StackCommands::Export { stack } => {
                let all = [mgr.list_local_stacks()?, mgr.featured_stacks()?].concat();
                let manifest = all
                    .into_iter()
                    .find(|s| s.id == stack || s.name.eq_ignore_ascii_case(&stack))
                    .ok_or_else(|| anyhow::anyhow!("Stack '{stack}' not found"))?;
                println!("{}", mgr.export_stack_toml(&manifest)?);
            }
            StackCommands::Create {
                name,
                author,
                description,
                skills,
            } => {
                let manifest =
                    mgr.create_stack_from_skills(&name, &author, &description, vec![], &skills)?;
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&manifest)?);
                } else {
                    println!(
                        "Created stack '{}' ({} skills)",
                        manifest.name,
                        manifest.skills.len()
                    );
                }
            }
        },
    }

    Ok(())
}
