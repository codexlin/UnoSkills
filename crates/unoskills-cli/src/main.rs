use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::json;
use std::path::PathBuf;
use std::process::ExitCode;
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
    /// Check upstream source and update an installed skill if changed
    Update { skill: String },
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
    /// Manage project-scope workspaces and repo-local skill deployments
    Project {
        #[command(subcommand)]
        action: ProjectCommands,
    },
    /// Check GitHub Releases for a newer UnoSkills version
    CheckUpdate,
    /// Sync plain-text skills & stacks via a private Git backup repository
    Backup {
        #[command(subcommand)]
        action: BackupCommands,
    },
}

#[derive(Subcommand)]
enum BackupCommands {
    /// Show configured Git backup repository, branch, and last sync timestamps
    Status,
    /// Configure the private Git backup repository URL and optional branch
    Configure {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        branch: Option<String>,
    },
    /// Mirror plain-text skills & stacks, commit changes, and push to remote
    Push,
    /// Pull plain-text skills & stacks from remote and rebuild the local SQLite index
    Pull,
}

#[derive(Subcommand)]
enum ProjectCommands {
    /// List attached project workspaces
    List,
    /// Attach a local project directory for repo-local skill deployment
    Add {
        path: PathBuf,
        #[arg(long)]
        name: Option<String>,
    },
    /// Detach a project workspace and undeploy its managed skills
    Remove {
        project: String,
        #[arg(long)]
        yes: bool,
    },
    /// Deploy a skill into a project workspace's agent skills directory
    Deploy {
        skill: String,
        #[arg(long)]
        project: String,
        #[arg(long = "agent", required = true)]
        agents: Vec<String>,
        #[arg(long)]
        copy: bool,
        #[arg(long)]
        force: bool,
    },
    /// Undeploy a skill from a project workspace's agent skills directory
    Undeploy {
        skill: String,
        #[arg(long)]
        project: String,
        #[arg(long = "agent", required = true)]
        agents: Vec<String>,
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
    /// Publish a stack as a secret GitHub Gist (uses the stored GitHub token)
    Share {
        stack: String,
        /// Use GitHub OAuth Device Flow instead of the stored PAT
        #[arg(long)]
        device: bool,
    },
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

fn main() -> ExitCode {
    let cli = Cli::parse();
    let json_mode = cli.json;
    match run(cli) {
        Ok(code) => code,
        Err(err) => {
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
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode> {
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
        Commands::Update { skill } => {
            let res = mgr.update_skill(&skill)?;
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({ "ok": true, "result": res }))?
                );
            } else if res.updated {
                println!(
                    "Updated '{}' (~{} tokens, {} copy deployment(s) refreshed)",
                    res.skill.name, res.skill.estimated_tokens, res.refreshed_copies
                );
                for issue in res.deployment_issues {
                    eprintln!("Review deployment at {}", issue.path.display());
                }
            } else {
                println!("'{}' is already up to date.", res.skill.name);
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
                let community = mgr.community_stacks().unwrap_or_default();
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &json!({ "featured": featured, "community": community, "local": local })
                        )?
                    );
                } else {
                    println!("=== Featured Stacks ===");
                    for s in featured {
                        println!("  {:<26} ({} skills) — {}", s.id, s.skills.len(), s.name);
                    }
                    println!("\n=== Community Stacks ===");
                    if community.is_empty() {
                        println!("  (unavailable offline — no cached index)");
                    } else {
                        for s in &community {
                            println!("  {:<26} ({} skills) — {}", s.id, s.skills.len(), s.name);
                        }
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
                    unoskills_core::stacks::parse_stack_from_toml(&text)?
                } else {
                    mgr.find_any_stack(&stack)?
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
                    for (skill, error) in &summary.failed {
                        eprintln!("Could not install {skill}: {error}");
                    }
                    for failure in &summary.deployment_failures {
                        eprintln!(
                            "Could not deploy {} to {}: {}",
                            failure.skill, failure.agent, failure.error
                        );
                    }
                }
                if !summary.failed.is_empty() || !summary.deployment_failures.is_empty() {
                    return Ok(ExitCode::FAILURE);
                }
            }
            StackCommands::Export { stack } => {
                let manifest = mgr
                    .find_any_stack(&stack)?
                    .ok_or_else(|| anyhow::anyhow!("Stack '{stack}' not found"))?;
                println!("{}", mgr.export_stack_toml(&manifest)?);
            }
            StackCommands::Share { stack, device } => {
                let manifest = mgr
                    .find_any_stack(&stack)?
                    .ok_or_else(|| anyhow::anyhow!("Stack '{stack}' not found"))?;
                let url = if device {
                    mgr.share_stack_via_device_flow(&manifest, |req| {
                        eprintln!(
                            "Open {} and enter code: {} (expires in {}s)",
                            req.verification_uri, req.user_code, req.expires_in
                        );
                    })?
                } else {
                    mgr.share_stack_to_gist(&manifest)?
                };
                if cli.json {
                    println!("{}", json!({ "ok": true, "gist_url": url }));
                } else {
                    println!("Shared '{stack}' as a secret Gist: {url}");
                }
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
        Commands::Project { action } => match action {
            ProjectCommands::List => {
                let projects = mgr.list_projects()?;
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&projects)?);
                } else if projects.is_empty() {
                    println!("No project workspaces attached. Try `unoskills project add <path>`.");
                } else {
                    for p in projects {
                        println!("{:<24} {}", p.name, p.path.display());
                    }
                }
            }
            ProjectCommands::Add { path, name } => {
                let proj = mgr.add_project(&path, name.as_deref())?;
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&json!({ "ok": true, "project": proj }))?
                    );
                } else {
                    println!(
                        "Attached project '{}' -> {}",
                        proj.name,
                        proj.path.display()
                    );
                }
            }
            ProjectCommands::Remove { project, yes } => {
                if !yes {
                    anyhow::bail!("Refusing to detach project '{project}' without --yes");
                }
                mgr.remove_project(&project)?;
                if cli.json {
                    println!("{}", json!({ "ok": true, "removed": project }));
                } else {
                    println!("Detached project '{project}'");
                }
            }
            ProjectCommands::Deploy {
                skill,
                project,
                agents,
                copy,
                force,
            } => {
                let mode = if copy { Some(DeployMode::Copy) } else { None };
                let mut results = Vec::new();
                for agent in &agents {
                    let dep = mgr.deploy_skill_to_project(&skill, &project, agent, mode, force)?;
                    results.push(dep);
                }
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &json!({ "ok": true, "project_deployments": results })
                        )?
                    );
                } else {
                    for dep in results {
                        println!(
                            "Deployed to project ({}, {:?}) -> {}",
                            dep.agent_key,
                            dep.mode,
                            dep.target_path.display()
                        );
                    }
                }
            }
            ProjectCommands::Undeploy {
                skill,
                project,
                agents,
            } => {
                for agent in &agents {
                    mgr.undeploy_skill_from_project(&skill, &project, agent)?;
                }
                if cli.json {
                    println!("{}", json!({ "ok": true, "undeployed": agents }));
                } else {
                    println!(
                        "Undeployed '{skill}' from project '{project}' ({})",
                        agents.join(", ")
                    );
                }
            }
        },
        Commands::CheckUpdate => {
            let outcome = mgr.check_app_update(env!("CARGO_PKG_VERSION"))?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&outcome)?);
            } else if outcome.update_available
                && let Some(release) = &outcome.latest
            {
                println!(
                    "Update available: v{} -> {}",
                    outcome.current_version, release.tag_name
                );
                if let Some(asset) = &outcome.recommended_asset {
                    println!("Installer: {}", asset.name);
                }
                if !release.html_url.is_empty() {
                    println!("Release:   {}", release.html_url);
                }
            } else {
                println!("UnoSkills v{} is up to date.", outcome.current_version);
            }
        }
        Commands::Backup { action } => match action {
            BackupCommands::Status => {
                let status = mgr.git_backup_status()?;
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&status)?);
                } else {
                    println!(
                        "Repository: {}",
                        status.repo_url.as_deref().unwrap_or("(not configured)")
                    );
                    println!("Branch:     {}", status.branch);
                    println!(
                        "Local data: {} skill(s), {} local stack(s)",
                        status.local_skill_count, status.local_stack_count
                    );
                    println!(
                        "Last push:  {}",
                        status
                            .last_pushed_at
                            .map(|t| t.to_string())
                            .unwrap_or_else(|| "-".to_string())
                    );
                    println!(
                        "Last pull:  {}",
                        status
                            .last_pulled_at
                            .map(|t| t.to_string())
                            .unwrap_or_else(|| "-".to_string())
                    );
                }
            }
            BackupCommands::Configure { repo, branch } => {
                mgr.configure_git_backup(&repo, branch.as_deref())?;
                let status = mgr.git_backup_status()?;
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&json!({ "ok": true, "status": status }))?
                    );
                } else {
                    println!(
                        "Configured Git backup: {} (branch: {})",
                        status.repo_url.as_deref().unwrap_or("(cleared)"),
                        status.branch
                    );
                }
            }
            BackupCommands::Push => {
                let res = mgr.git_backup_push()?;
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&json!({ "ok": true, "result": res }))?
                    );
                } else if res.pushed {
                    println!(
                        "Pushed {} skill(s) and {} stack(s) (commit {}).",
                        res.skills_exported,
                        res.stacks_exported,
                        res.commit_sha.as_deref().unwrap_or("-")
                    );
                } else {
                    println!("Already up to date — nothing new to push.");
                }
            }
            BackupCommands::Pull => {
                let res = mgr.git_backup_pull()?;
                let complete = res.conflicts.is_empty();
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&json!({ "ok": complete, "result": res }))?
                    );
                } else {
                    println!(
                        "Pulled backup: {} imported, {} updated, {} unchanged skill(s); {} stack(s); {} copy deployment(s) refreshed.",
                        res.skills_imported,
                        res.skills_updated,
                        res.skills_unchanged,
                        res.stacks_imported,
                        res.refreshed_copies
                    );
                    for conflict in &res.conflicts {
                        eprintln!("{conflict}");
                    }
                }
                if !complete {
                    return Ok(ExitCode::FAILURE);
                }
            }
        },
    }

    Ok(ExitCode::SUCCESS)
}
