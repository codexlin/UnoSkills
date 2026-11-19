---
name: unoskills
description: Manage the user's shared AI agent skill library and curated Skill Stacks via the `unoskills` CLI — list, inspect, install, adopt existing skills, deploy/undeploy across agents safely, and share or install declarative Skill Stacks.
---

# UnoSkills CLI Skill

Use `unoskills --json ...` whenever inspecting or mutating the user's skill library so output is structured and safe.

## Common Commands

```bash
# Inspect installed skills & detected agents
unoskills --json list
unoskills --json agents
unoskills --json show <skill-slug>

# Install a skill into ~/.unoskills/skills/ (library-only by default)
unoskills --json install vercel-labs/agent-skills@vercel-react-best-practices
unoskills --json install https://github.com/owner/repo/tree/main/skills/my-skill
unoskills --json install ./local-skill

# Deploy / undeploy explicitly per agent (safe NoClobber + shared-dir protection)
unoskills --json deploy <skill-slug> --agent claude_code --agent codex
unoskills --json undeploy <skill-slug> --agent codex

# Discover & adopt unmanaged skills already inside agent folders
unoskills --json adopt --dry-run
unoskills --json adopt

# Curated & shareable Skill Stacks
unoskills --json stack list
unoskills --json stack install frontend-react-ship --agent claude_code
unoskills stack export frontend-react-ship
```

## Safety Rules

- Never pass `--force` to `deploy` without first showing the conflicting path to the user and getting confirmation.
- Always run `adopt --dry-run` before `adopt`.
- Always confirm with the user before passing `--yes` to `unoskills remove`.
