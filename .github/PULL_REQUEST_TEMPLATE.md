## Summary

<!-- Describe what this PR changes and why. -->

## Type of Change

- [ ] Bug fix
- [ ] New feature
- [ ] Native Shell desktop UI
- [ ] Refactor / performance improvement
- [ ] Agent registry (`crates/unoskills-core/data/agents.toml`) or Featured Stack (`crates/unoskills-core/data/featured_stacks.toml`)
- [ ] Docs / CI / Chore

## AGENTS.md Checklist

- [ ] Follows [engineering rules](../AGENTS.md) and [desktop design standards](../DESIGN.md)
- [ ] Uses the active preset's color pairs and verifies Light, Dark, and System modes for theme changes
- [ ] Fits the minimum and reference window sizes (`960 × 600`, `1240 × 780`)
- [ ] Keeps user-facing strings in the shared or feature locale tables (zh-CN, en, ja)
- [ ] Core filesystem safety invariants preserved (`SKILL.md`-only, `ReplacePolicy`, shared-dir protection)
- [ ] Rust tests, formatting, and Clippy pass
- [ ] NSH type check, release bundle, and headless tests pass for desktop changes

## Screenshots (if UI changed)

<!-- Attach before/after screenshots here -->
