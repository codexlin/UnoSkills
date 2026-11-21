# Contributing to UnoSkills

Thanks for helping grow **UnoSkills**!

## Add a New AI Agent

Edit `crates/unoskills-core/data/agents.toml` and append an `[[agent]]` entry:

```toml
[[agent]]
key = "my_agent"
display_name = "My Agent"
category = "coding"
relative_skills_dir = ".my-agent/skills"
relative_detect_dir = ".my-agent"
```

Run `cargo test -p unoskills-core` to verify the registry parses cleanly.

## Submit a Curated Skill Stack

1. Build your stack inside the **UnoSkills** app (`Stacks -> Create / Share`) and click **Copy Share TOML** (or run `unoskills stack export <id>`).
2. Add your stack to `crates/unoskills-core/data/featured_stacks.toml` (or open a PR to the community stacks repository).
3. Include clear per-skill pairing notes explaining *why* those skills work well together.

## Development

```bash
cargo test -p unoskills-core
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run -p unoskills-app --locked
nsh check crates/unoskills-app/ui
nsh build crates/unoskills-app/ui --release
nsh test crates/unoskills-app/ui
```

Use the NSH 0.6.1 CLI for the last three commands. Desktop UI changes should include evidence from the affected workflows in the native app. Use temporary libraries for tests that deploy or edit files. [Engineering rules](AGENTS.md) · [Project structure and verification](docs/ARCHITECTURE.md)
