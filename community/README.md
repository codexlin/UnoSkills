# Community Stacks Index

This directory hosts the **Stage 1 remote community index** consumed by UnoSkills at runtime:

```
https://raw.githubusercontent.com/codexlin/UnoSkills/main/community/index.json
```

Every UnoSkills app fetches this file on startup (with ETag caching and an offline
cached fallback), so a merged PR here is live for all users **without shipping an
app release**.

## Submit a Stack

1. Compose your stack locally: in the app's **Stacks → Create / Share** view,
   select skills, fill in name/author/description, then **Copy share TOML**.
   (Or hand-write a `.unoskills-stack.toml` — same schema.)
2. Append a JSON object to [`index.json`](./index.json) with the same fields:

```json
{
  "id": "kebab-case-unique-id",
  "name": "Human Readable Name",
  "author": "your-github-handle",
  "description": "One sentence: what this combo is for.",
  "tags": ["relevant", "tags"],
  "recommended_agents": ["claude_code", "codex"],
  "skills": [
    { "name": "skill-name", "source": "owner/repo@skill", "note": "Optional note" }
  ]
}
```

3. Open a pull request. CI validates the `index.json` schema, non-empty skill
   entries, and unique stack `id`s (including zero collisions with built-in
   featured stacks). Reviewers check:
   - every skill ref is publicly installable (`owner/repo@skill` or git URL) and resolves via UnoSkills' strict `SKILL.md` detection
   - no duplicate `id`s
   - description says what the combo is *for*, not a skill list

## Rules

- **Stacks reference skills; they never copy skill code.** A stack is a recipe,
  so contributors only hold the copyright to their own manifest.
- Skills must have a canonical `SKILL.md` entry (`README.md`/`CLAUDE.md` are not
  skill entry markers and will be rejected).
- `verified: true` is reserved for stacks vetted by the UnoSkills maintainers;
  community submissions leave it out.
- Keep stacks small and opinionated — 2 to 6 skills works best.
