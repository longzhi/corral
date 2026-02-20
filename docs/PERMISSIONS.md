# Permissions Guide

This guide explains Corral's permission model and how to write `skill.yaml` manifests.

## Philosophy

Corral uses a **declarative, default-deny permission model**:

- Skills **declare** what they need in `skill.yaml`
- The system **enforces** these permissions at runtime
- **Everything is denied** unless explicitly allowed
- Users can **review and approve** permissions before installation

This model is inspired by:
- Mobile app permissions (iOS/Android)
- Flatpak/Snap permission portals
- Deno's capability-based security

## skill.yaml Format

### Basic Structure

```yaml
# Metadata
name: my-skill
version: 1.0.0
description: What this skill does
author: your-name
entry: ./run.sh
runtime: bash  # bash | python | node

# Permissions
permissions:
  fs:
    read: []
    write: []
  network:
    allow: []
  services: {}
  exec: []
  env: []
```

### Complete Example

```yaml
name: smart-shopping
version: 1.0.0
description: Manage shopping lists with calendar reminders
author: community/alice
entry: ./run.sh
runtime: bash

permissions:
  fs:
    read:
      - $SKILL_DIR/**           # Skill's own files
      - $DATA_DIR/config.json   # Persistent config
    write:
      - $WORK_DIR/**            # Temporary working directory
      - $DATA_DIR/**            # Persistent data

  network:
    allow:
      - api.example.com:443     # HTTPS API
      - cdn.example.com:443     # CDN for assets

  services:
    reminders:
      access: readwrite
      scope:
        lists: ["Shopping"]
    calendar:
      access: read
      scope:
        calendars: ["*"]
        range: 7d
    browser:
      access: open
      scope:
        domains: ["example.com", "*.example.com"]
    notifications:
      access: send

  exec:
    - curl
    - jq
    - python3

  env:
    - LANG
    - TZ
    - SKILL_API_KEY
```

## Permission Types

### File System Permissions

Control which files and directories a skill can access.

```yaml
permissions:
  fs:
    read:
      - $SKILL_DIR/**           # Everything in skill directory
      - $DATA_DIR/config.json   # Specific file
      - /etc/hosts              # System file (rarely needed)
    write:
      - $WORK_DIR/**            # Temporary workspace
      - $DATA_DIR/**            # Persistent storage
```

**Supported Variables:**

| Variable | Path | Lifetime | Access |
|----------|------|----------|--------|
| `$SKILL_DIR` | Skill installation directory | Persistent | Read-only |
| `$DATA_DIR` | Skill's persistent data | Persistent | Read-write |
| `$WORK_DIR` | Temporary working directory | Per-run | Read-write |
| `$SHARED_DIR` | Shared between skills | Persistent | Controlled |

**Glob Patterns:**

- `**` — Match all subdirectories recursively
- `*` — Match files in current directory
- `?` — Match single character
- `[abc]` — Match any of a, b, c

**Examples:**

```yaml
fs:
  read:
    - $SKILL_DIR/**/*.json      # All JSON files
    - $DATA_DIR/cache/*.txt     # Specific directory
    - $DATA_DIR/data-?.csv      # data-1.csv, data-2.csv, etc.
  write:
    - $WORK_DIR/output/**       # Output directory tree
```

### Network Permissions

Control which domains and ports a skill can connect to.

```yaml
permissions:
  network:
    allow:
      - api.example.com:443           # Specific domain and port
      - example.com:*                 # Any port on domain
      - "*.example.com:443"           # Wildcard subdomain
      - 192.168.1.100:8080            # IP address
```

**Wildcards:**

- `*.example.com` — Matches `api.example.com`, `cdn.example.com`, etc.
- `example.com:*` — Any port on the domain
- `*:443` — Any domain on port 443 (not recommended)

**Best Practices:**

- Use HTTPS (port 443) when possible
- Be specific about domains
- Avoid wildcards unless necessary
- Document why each domain is needed

### Service Permissions

Control access to system services (reminders, calendar, browser, etc.).

#### Reminders

```yaml
permissions:
  services:
    reminders:
      access: readwrite  # read | write | readwrite
      scope:
        lists: ["Shopping", "Work"]  # Specific lists only
        # OR
        lists: ["*"]                 # All lists
```

**Access Levels:**
- `read` — List and view reminders only
- `write` — Create/update/delete (implies read)
- `readwrite` — Explicit read and write

**Available Methods:**
- `reminders.list` — List reminders (filters: list, completed)
- `reminders.add` — Create a reminder
- `reminders.update` — Update a reminder
- `reminders.complete` — Mark as completed
- `reminders.delete` — Delete a reminder

**Example Usage:**

```bash
# List incomplete reminders
sandbox-call reminders.list --list Shopping --completed false

# Add a reminder
sandbox-call reminders.add \
  --list Shopping \
  --title "Buy milk" \
  --dueDate "2025-02-10T18:00:00+08:00" \
  --notes "Organic preferred"

# Complete a reminder
sandbox-call reminders.complete --id "EK:xxx"
```

#### Calendar (Planned)

```yaml
permissions:
  services:
    calendar:
      access: readwrite
      scope:
        calendars: ["*"]        # All calendars
        range: 30d              # Next 30 days
        # OR
        calendars: ["Work", "Personal"]
        range: 7d               # Next 7 days
```

**Access Levels:**
- `read` — View events only
- `write` — Create/update/delete events (implies read)
- `readwrite` — Explicit read and write

**Scope Parameters:**
- `calendars` — Array of calendar names or `["*"]` for all
- `range` — Time range (e.g., `7d`, `30d`, `90d`)

#### Browser

```yaml
permissions:
  services:
    browser:
      access: open
      scope:
        domains: ["example.com", "*.example.com"]
```

**Access Levels:**
- `open` — Open URLs in default browser

**Scope:**
- `domains` — Allowed domains (supports wildcards)

**Example Usage:**

```bash
sandbox-call browser.open --url "https://example.com/page"
```

#### Notifications

```yaml
permissions:
  services:
    notifications:
      access: send
```

**Access Levels:**
- `send` — Send notifications

**Example Usage:**

```bash
sandbox-call notifications.send \
  --title "Task Complete" \
  --body "Shopping list updated" \
  --sound "default"
```

#### Clipboard (Planned)

```yaml
permissions:
  services:
    clipboard:
      access: readwrite
```

**Access Levels:**
- `read` — Read clipboard contents
- `write` — Write to clipboard
- `readwrite` — Both read and write

### Process Execution

Control which external programs a skill can execute.

```yaml
permissions:
  exec:
    - curl
    - jq
    - python3
    - /usr/bin/ffmpeg
```

**Notes:**
- Program names are resolved via `$PATH`
- Absolute paths are also supported
- Each executable must be explicitly listed
- Arguments are not restricted once exec is allowed

**Example Usage:**

```bash
# From within skill script
curl -s https://api.example.com/data | jq '.results[]'
python3 process.py input.json
```

### Environment Variables

Control which environment variables a skill can access.

```yaml
permissions:
  env:
    - LANG
    - TZ
    - HOME
    - SKILL_API_KEY
```

**Default Variables:**

These are always available:
- `SKILL_DIR` — Skill installation directory
- `DATA_DIR` — Persistent data directory
- `WORK_DIR` — Temporary working directory
- `PATH` — Restricted to include only `sandbox-call`

**Custom Variables:**

Any other variables must be explicitly listed.

## Permission Approval

### Interactive Approval

When running a skill for the first time:

```bash
$ corral run --skill ./my-skill

📦 Installing "my-skill" v1.0.0 by community/alice

Permissions requested:
  📁 File Access
     Read:  skill files, config.json
     Write: working dir, persistent data
  🌐 Network
     api.example.com:443
     cdn.example.com:443
  📋 Reminders
     Read & Write list: "Shopping"
  📅 Calendar
     Read only (next 7 days)
  🌍 Browser
     Open URLs on: example.com
  🔔 Notifications
     Send notifications

[A]llow  [D]eny  [R]eview manifest  [?] Show details
```

### Pre-approval

Approve permissions before running:

```bash
corral approve --skill ./my-skill
```

### Inspecting Permissions

View permissions without running:

```bash
corral inspect --skill ./my-skill
```

## Security Best Practices

### For Skill Authors

1. **Request minimum permissions** — Only ask for what you need
2. **Document why** — Explain why each permission is needed
3. **Use specific scopes** — Don't request `["*"]` unless necessary
4. **Prefer read-only** — Request write access only when needed
5. **Version carefully** — Changing permissions = new review

### For Skill Users

1. **Review before approving** — Read the permission list carefully
2. **Check the author** — Is this from a trusted source?
3. **Understand the scope** — What can this skill actually do?
4. **Deny suspicious requests** — If it seems excessive, deny it
5. **Report malicious skills** — Help keep the ecosystem safe

## Common Patterns

### Read-Only Data Processor

```yaml
permissions:
  fs:
    read:
      - $SKILL_DIR/**
      - $DATA_DIR/input/**
    write:
      - $WORK_DIR/**
  exec:
    - jq
    - python3
```

### API Client

```yaml
permissions:
  fs:
    read:
      - $SKILL_DIR/**
      - $DATA_DIR/config.json
    write:
      - $DATA_DIR/cache/**
  network:
    allow:
      - api.example.com:443
  services:
    notifications:
      access: send
```

### System Service Integrator

```yaml
permissions:
  services:
    reminders:
      access: readwrite
      scope:
        lists: ["Shopping"]
    calendar:
      access: read
      scope:
        calendars: ["*"]
        range: 7d
    notifications:
      access: send
```

### File Processor with Output

```yaml
permissions:
  fs:
    read:
      - $SKILL_DIR/**
      - $DATA_DIR/input/**
    write:
      - $DATA_DIR/output/**
  exec:
    - ffmpeg
    - imagemagick
```

## Error Messages

### Permission Denied

```json
{
  "error": {
    "code": -32001,
    "message": "Permission denied",
    "data": {
      "service": "reminders",
      "reason": "Service not declared in manifest"
    }
  }
}
```

**Solution:** Add the service to your `skill.yaml` permissions.

### Scope Violation

```json
{
  "error": {
    "code": -32002,
    "message": "Scope violation",
    "data": {
      "service": "reminders",
      "requested_list": "Work",
      "allowed_lists": ["Shopping"]
    }
  }
}
```

**Solution:** Update your manifest scope or request a different list.

### Network Denied

```json
{
  "error": {
    "code": -32006,
    "message": "Network access denied",
    "data": {
      "domain": "evil.com",
      "allowed_domains": ["api.example.com"]
    }
  }
}
```

**Solution:** Add the domain to your `network.allow` list.

### Path Denied

```json
{
  "error": {
    "code": -32007,
    "message": "Path access denied",
    "data": {
      "path": "/etc/passwd",
      "access": "read",
      "allowed_patterns": ["$SKILL_DIR/**", "$DATA_DIR/**"]
    }
  }
}
```

**Solution:** Add the path to your `fs.read` or `fs.write` list.

## Advanced Topics

### Dynamic Permissions

Currently, permissions are **static** — declared at install time and cannot change during execution.

**Future:** Support for runtime permission requests with user prompts.

### Permission Inheritance

Skills cannot inherit permissions from other skills. Each skill must declare its own permissions.

### Shared Resources

The `$SHARED_DIR` variable allows controlled sharing between skills, but requires explicit permission:

```yaml
permissions:
  fs:
    read:
      - $SHARED_DIR/public/**
    write:
      - $SHARED_DIR/my-skill-data/**
```

### Capability Delegation

A skill can be granted the ability to run other skills (future feature).

## Reference

### Complete Permission Schema

```yaml
permissions:
  fs:
    read: [string]      # Array of glob patterns
    write: [string]     # Array of glob patterns
  
  network:
    allow: [string]     # Array of "domain:port" or "*.domain:port"
  
  services:
    reminders:
      access: string    # "read" | "write" | "readwrite"
      scope:
        lists: [string] # Array of list names or ["*"]
    
    calendar:
      access: string    # "read" | "write" | "readwrite"
      scope:
        calendars: [string]  # Array of calendar names or ["*"]
        range: string        # e.g., "7d", "30d", "90d"
    
    browser:
      access: string    # "open"
      scope:
        domains: [string]    # Array of allowed domains
    
    notifications:
      access: string    # "send"
    
    clipboard:
      access: string    # "read" | "write" | "readwrite"
  
  exec: [string]        # Array of executable names or paths
  
  env: [string]         # Array of environment variable names
```

### Error Code Reference

| Code | Name | Description |
|------|------|-------------|
| -32001 | Permission Denied | Service/resource not declared in manifest |
| -32002 | Scope Violation | Request exceeds declared scope |
| -32003 | Rate Limited | Too many requests in time window |
| -32004 | Timeout | Operation exceeded timeout limit |
| -32005 | Service Unavailable | Platform does not support this service |
| -32006 | Network Denied | Domain not in allowed list |
| -32007 | Path Denied | Path not in allowed patterns |

## See Also

- [Architecture Overview](ARCHITECTURE.md) — Component details
- [Design Document](DESIGN.md) — Technical design
- [Contributing Guide](../CONTRIBUTING.md) — Development setup
