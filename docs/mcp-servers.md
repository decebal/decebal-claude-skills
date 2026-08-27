# MCP Server Integrations

Model Context Protocol servers connect Claude to external services. Configured at `~/.claude/plugins/marketplaces/claude-plugins-official/external_plugins/`.

## Active Integrations

### Project Management
| Server | Transport | Auth | Use Case |
|--------|-----------|------|----------|
| **Linear** | HTTP | OAuth | Issue tracking, project management |
| **Asana** | HTTP | OAuth | Task management |

### Code & DevOps
| Server | Transport | Auth | Use Case |
|--------|-----------|------|----------|
| **GitHub** | HTTP | Bearer token (`GITHUB_PERSONAL_ACCESS_TOKEN`) | PRs, issues, repos |
| **GitLab** | HTTP | OAuth | PRs, issues, repos |
| **Supabase** | HTTP | OAuth | Database management |
| **Firebase** | stdio (npx) | OAuth | Firebase project management |

### Communication
| Server | Transport | Auth | Use Case |
|--------|-----------|------|----------|
| **Slack** | HTTP | OAuth (port 3118) | Channel messaging |

### Payments & Business
| Server | Transport | Auth | Use Case |
|--------|-----------|------|----------|
| **Stripe** | HTTP | OAuth | Payment operations |

### Developer Tools
| Server | Transport | Auth | Use Case |
|--------|-----------|------|----------|
| **Playwright** | HTTP | None | Browser automation |
| **Context7** | HTTP | None | Documentation search |
| **Greptile** | HTTP | API key | Codebase search |
| **Serena** | HTTP | None | Code navigation |

## Configuration Format

MCP servers are configured as `.mcp.json` files. Example for GitHub:

```json
{
  "type": "http",
  "url": "https://api.githubcopilot.com/mcp/",
  "headers": {
    "Authorization": "Bearer ${GITHUB_PERSONAL_ACCESS_TOKEN}"
  }
}
```

Example for Firebase (stdio transport):

```json
{
  "type": "command",
  "command": "npx",
  "args": ["-y", "firebase-tools@latest", "mcp"]
}
```

## Auth Cache

Services requiring authentication are tracked in `~/.claude/mcp-needs-auth-cache.json`. Claude prompts for auth on first use.

## Adding New MCP Servers

1. Create a `.mcp.json` file in the external plugins directory
2. Or use the `mcp-builder` skill to create a custom server
3. For project-specific servers, configure in the project's `.claude/` directory

## Local Log Server

A high-value local pattern for desktop/background apps: a stdio MCP server that
lets Claude read application logs at runtime (`list_logs`, `tail_log`,
`get_errors`, `search_logs`, `log_stats`). See
[mcp-log-server.md](mcp-log-server.md).
