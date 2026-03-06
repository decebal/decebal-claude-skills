# Permission Patterns & Security

Claude Code uses a granular permission system. Configure per-project in `.claude/settings.local.json`.

## Permission Format

```json
{
  "permissions": {
    "allow": [
      "Bash(command pattern *)"
    ]
  }
}
```

Wildcards (`*`) match any arguments. Be specific to minimize surface area.

## Permission Tiers by Project Type

### Minimal (docs, static sites)
```json
{
  "permissions": {
    "allow": [
      "Bash(ls *)",
      "Bash(pwd)",
      "Bash(git status *)",
      "Bash(git diff *)",
      "Bash(git log *)"
    ]
  }
}
```

### Standard (Bun/Node projects)
```json
{
  "permissions": {
    "allow": [
      "Bash(bun *)",
      "Bash(bunx *)",
      "Bash(task *)",
      "Bash(git *)",
      "Bash(gh *)",
      "Bash(ls *)",
      "Bash(pwd)",
      "Bash(wc *)",
      "Bash(head *)",
      "Bash(cat *)"
    ]
  }
}
```

### Full Development (Rust + Docker + CI)
```json
{
  "permissions": {
    "allow": [
      "Bash(bun *)",
      "Bash(bunx *)",
      "Bash(task *)",
      "Bash(cargo *)",
      "Bash(rustup *)",
      "Bash(docker compose *)",
      "Bash(docker build *)",
      "Bash(docker ps *)",
      "Bash(docker logs *)",
      "Bash(git *)",
      "Bash(gh *)",
      "Bash(curl *)",
      "Bash(ls *)",
      "Bash(pwd)",
      "Bash(wc *)",
      "Bash(head *)",
      "Bash(cat *)",
      "Bash(mix *)",
      "Bash(go *)"
    ]
  }
}
```

## Security Principles

1. **Allowlist, not blocklist** — Only permit specific commands
2. **Project-scoped** — Each project gets its own permission set
3. **No secrets in permissions** — Environment variables stay in `.env` files
4. **SOPS for encrypted secrets** — Use `sops` for secrets that must be version-controlled
5. **Review periodically** — Remove commands no longer needed

## What NOT to Allow

- `Bash(rm -rf *)` — Destructive filesystem operations
- `Bash(sudo *)` — Elevated privileges
- `Bash(ssh *)` — Remote access (unless specifically needed)
- `Bash(env)` / `Bash(printenv)` — Can leak secrets
- `Bash(curl * | sh)` — Remote code execution

## Per-Project Setup

```bash
# Create project config directory
mkdir -p .claude

# Create settings file
cat > .claude/settings.local.json << 'EOF'
{
  "permissions": {
    "allow": [
      "Bash(bun *)",
      "Bash(task *)",
      "Bash(git *)"
    ]
  }
}
EOF
```
