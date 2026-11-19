## Summary

<!-- Describe what this PR changes and why. -->

## Type of Change

- [ ] Bug fix
- [ ] New feature
- [ ] UI / GPUI Kit polish
- [ ] Refactor / performance improvement
- [ ] Agent registry (`crates/unoskills-core/data/agents.toml`) or Featured Stack (`crates/unoskills-core/data/featured_stacks.toml`)
- [ ] Docs / CI / Chore

## AGENTS.md Checklist

- [ ] Follows `AGENTS.md` GPUI Kit Design / Style / Coding / Scrollable guidelines
- [ ] Uses `cx.theme()` semantic tokens (verified in both Dark & Light modes)
- [ ] No horizontal overflow at default window size (`1240x780`)
- [ ] All user-facing strings added to `i18n.rs` (zh-CN / en / ja parity)
- [ ] Core filesystem safety invariants preserved (`SKILL.md`-only, `ReplacePolicy`, shared-dir protection)
- [ ] `cargo test -p unoskills-core` passes
- [ ] `cargo check --workspace` passes with **0 errors and 0 warnings**

## Screenshots (if UI changed)

<!-- Attach before/after screenshots here -->
