# Plugins & LSP Configuration

## Installed LSP Plugins

Language Server Protocol plugins give Claude real-time code intelligence.

| Plugin | Language | Install |
|--------|----------|---------|
| `rust-analyzer-lsp@claude-plugins-official` | Rust | `claude plugins install rust-analyzer-lsp` |
| `gopls-lsp@claude-plugins-official` | Go | `claude plugins install gopls-lsp` |

Other available LSPs: `typescript-lsp`, `pyright-lsp`, `clangd-lsp`, `jdtls-lsp`, `kotlin-lsp`, `lua-lsp`, `php-lsp`, `swift-lsp`, `csharp-lsp`.

## Enabled Plugins

Enable in `~/.claude/settings.json`:

```json
{
  "enabledPlugins": {
    "rust-analyzer-lsp@claude-plugins-official": true,
    "gopls-lsp@claude-plugins-official": true
  }
}
```

## Plugin Blocklist

Block problematic plugins in `~/.claude/plugins/blocklist.json`:

```json
[
  {
    "pluginId": "code-review@claude-plugins-official",
    "reason": "conflicts with custom workflow"
  }
]
```

## Useful Development Plugins

Available in the official marketplace (`claude-plugins-official`):

| Plugin | Purpose |
|--------|---------|
| `code-review` | Automated code review feedback |
| `code-simplifier` | Reduce code complexity |
| `commit-commands` | Git commit helpers |
| `feature-dev` | Feature development workflow |
| `pr-review-toolkit` | Pull request review |
| `security-guidance` | Security checks before file edits |
| `hookify` | Hook management |
| `frontend-design` | Frontend design patterns |

## Status Line

Custom status line using Bun:

```json
{
  "statusLine": {
    "type": "command",
    "command": "bunx ccstatusline@latest",
    "padding": 0
  }
}
```
