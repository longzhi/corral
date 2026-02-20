# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Security Model

Corral is designed to sandbox untrusted community scripts and agent skills. Our threat model assumes:

**What we protect against:**
- Buggy or casually malicious scripts
- Accidental data leakage
- Unauthorized file access
- Unexpected network connections
- Unwanted system service calls
- Resource exhaustion (memory/CPU)

**What we don't protect against:**
- Advanced Persistent Threats (APTs)
- Kernel-level exploits
- Direct syscall abuse in compiled binaries
- Social engineering attacks on users
- Physical access to the machine

## Platform Security

### macOS
- **Isolation Method:** DYLD_INSERT_LIBRARIES (user-space interposition)
- **Security Level:** Medium — Can be bypassed by direct syscalls
- **Best For:** Interpreted scripts (bash, Python, Node.js)
- **Not Recommended For:** Untrusted compiled binaries

### Linux
- **Isolation Method:** bubblewrap (kernel namespaces) or LD_PRELOAD
- **Security Level:** High (bubblewrap) / Medium (LD_PRELOAD)
- **Best For:** All script types; bubblewrap provides strong isolation
- **Note:** bubblewrap provides kernel-level isolation, much stronger than user-space

### Windows
- **Status:** Planned (not yet implemented)
- **Planned Method:** Restricted Token + Job Objects + Detours
- **Security Level:** Medium-High

## Known Limitations

### 1. User-Space Interposition (macOS, Linux LD_PRELOAD)

The C interposition library can be bypassed by:
- Direct syscalls (bypassing libc)
- Custom compiled binaries with inline syscalls
- JIT compilation with embedded syscalls

**Mitigation:** 
- Only run interpreted scripts from trusted sources
- Use bubblewrap on Linux for stronger isolation
- Review skill source code before running

### 2. Time-of-Check/Time-of-Use (TOCTOU)

Path validation has inherent race conditions between:
- Checking if a path is allowed
- Actually opening the file

**Mitigation:**
- Minimize privileges overall
- Use atomic operations where possible
- Accept this as a limitation of user-space enforcement

### 3. Information Disclosure

Certain error messages may leak information about:
- File system structure
- Network topology
- Installed services

**Mitigation:**
- Generic error messages in production
- Detailed errors only in development mode
- Rate limiting to prevent enumeration

### 4. Resource Limits

Current resource controls are basic:
- Timeouts work well
- Memory limits are process-level
- No disk quota enforcement yet

**Mitigation:**
- Set reasonable timeout values
- Monitor disk usage externally
- Future: Add disk quota support

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, please report them privately:

1. **Email:** security@yourdomain.com (replace with your actual email)
2. **Subject:** `[SECURITY] Corral Vulnerability Report`
3. **Include:**
   - Description of the vulnerability
   - Steps to reproduce
   - Affected versions
   - Potential impact
   - Suggested fix (if any)

### Response Timeline

- **Initial Response:** Within 48 hours
- **Status Update:** Within 7 days
- **Fix Timeline:** Depends on severity
  - Critical: 7-14 days
  - High: 14-30 days
  - Medium: 30-60 days
  - Low: Next release cycle

### Disclosure Policy

- We follow **coordinated disclosure**
- Public disclosure after fix is available
- Credit given to reporters (if desired)
- CVE assigned for significant vulnerabilities

## Security Best Practices

### For Skill Authors

1. **Request Minimum Permissions**
   - Only ask for what you absolutely need
   - Use read-only access when possible
   - Limit network domains to specific APIs

2. **Validate Input**
   - Never trust user input
   - Sanitize all data before use
   - Use parameterized queries

3. **Secure Secrets**
   - Use environment variables for API keys
   - Never hardcode credentials
   - Use encrypted storage when available

4. **Handle Errors Gracefully**
   - Don't leak sensitive info in error messages
   - Log errors appropriately
   - Fail securely

### For Skill Users

1. **Review Permissions**
   - Read the permission list carefully
   - Understand what each permission allows
   - Deny excessive or suspicious requests

2. **Check the Source**
   - Prefer skills from trusted authors
   - Review the source code if possible
   - Check for community reviews

3. **Monitor Behavior**
   - Check audit logs periodically
   - Watch for unexpected network activity
   - Report suspicious behavior

4. **Keep Updated**
   - Update Corral regularly
   - Update skills to latest versions
   - Subscribe to security announcements

## Security Features

### Default Deny

Everything is denied unless explicitly allowed:
- File access → Must be in `fs.read` or `fs.write`
- Network → Must be in `network.allow`
- Services → Must be in `services.*`
- Process exec → Must be in `exec`

### Audit Logging

All broker calls are automatically logged:
- What service was called
- With what parameters
- Whether it was allowed/denied
- How long it took
- Exit status

Logs are in JSONL format for easy analysis.

### Scope Enforcement

Even when a service is allowed, scope limits what can be accessed:
- Reminders: Specific lists only
- Calendar: Specific calendars and date ranges
- Browser: Specific domains only

### Resource Monitoring

Basic resource controls:
- Execution timeouts
- Memory limits (process-level)
- Rate limiting (planned)

## Secure Development

### Code Review

- All PRs require review
- Security-sensitive changes need extra scrutiny
- Automated checks via CI (clippy, fmt, tests)

### Dependencies

- Minimize external dependencies
- Audit dependencies regularly (`cargo audit`)
- Pin versions in production
- Update dependencies promptly

### Testing

- Unit tests for all critical paths
- Integration tests for end-to-end flows
- Security-specific tests for permission enforcement
- Fuzzing (planned)

## Security Roadmap

### v0.2.0
- [ ] Improved error messages (less info leakage)
- [ ] Disk quota enforcement
- [ ] Enhanced rate limiting

### v0.3.0
- [ ] Fuzzing infrastructure
- [ ] Security audit of C code
- [ ] Windows security review

### v0.4.0
- [ ] SELinux/AppArmor integration (Linux)
- [ ] Code signing for helpers
- [ ] Encrypted skill storage

## Contact

For security-related questions or concerns:
- **Email:** security@yourdomain.com
- **PGP Key:** [Link to your PGP key]

For general issues, use GitHub Issues.

## Hall of Fame

We appreciate security researchers who help make Corral more secure. Contributors will be listed here (with permission):

<!-- Security researchers who have responsibly disclosed vulnerabilities -->

---

**Last Updated:** 2025-02-20
