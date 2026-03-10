# Security Policy

## Supported Versions

| Version | Supported          |
|---------|--------------------|
| 0.0.1   | :white_check_mark: |
| < 0.0.1 | :x:                |

## Reporting a Vulnerability

We take security seriously. If you discover a security vulnerability in Skill Router, please follow these guidelines:

### Reporting Process

1. **Do NOT** create a public issue
2. **Send an email** to: `security@aionui.org`
3. **Include**:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if known)
4. **Wait for** confirmation from the security team

### Response Timeline

- **Initial Response**: Within 48 hours
- **Detailed Analysis**: Within 7 days
- **Fix Timeline**: Based on severity
- **Public Disclosure**: After fix is deployed

### What Happens Next?

1. Our security team will verify the vulnerability
2. We'll develop a fix
3. Coordinate on disclosure timeline
4. Credit you in the security advisory

## Security Features

### Defense in Depth

Skill Router implements multiple layers of security:

#### 1. Permission System

```rust
pub struct Permissions {
    pub network: bool,           // Network access
    pub filesystem_read: bool,   // File system read
    pub filesystem_write: bool,  // File system write
    pub process_exec: bool,      // Process execution
}
```

- **Default Deny**: All permissions default to `false`
- **Explicit Authorization**: Skills must declare required permissions
- **Runtime Validation**: Checked before every execution

#### 2. Skill Isolation

- Skills run in separate processes
- No direct memory access to core system
- Input/output through stdin/stdout only

#### 3. Audit Logging

```rust
pub struct Usage {
    pub total_calls: u64,
    pub success_calls: u64,
    pub failed_calls: u64,
    pub avg_latency_ms: f64,
    pub last_used: String,
}
```

- Comprehensive execution logging
- Performance metrics tracking
- Failed operation records

#### 4. Repository Scanning

Automatic security analysis of downloaded skills:

```rust
pub struct SecurityAnalyzer {
    // Analyzes skill directories for vulnerabilities
    pub fn audit_skill_dir(path: &Path) -> Result<()>
}
```

#### 5. Input Validation

- JSON schema validation for all inputs
- Type checking for all parameters
- Sanitization of external data

### Security Best Practices

#### For Users

1. **Review Permissions**: Always check skill permissions before execution
2. **Keep Updated**: Use latest version of Skill Router
3. **Trusted Sources**: Only install skills from trusted repositories
4. **Audit Logs**: Regularly review execution logs
5. **Network Access**: Minimize network permissions for sensitive tasks

#### For Skill Developers

1. **Principle of Least Privilege**: Request minimum necessary permissions
2. **Input Validation**: Validate all user inputs
3. **Error Handling**: Don't expose sensitive information in errors
4. **Dependencies**: Keep dependencies updated
5. **Testing**: Include security tests in skill development

#### Example Secure Skill

```json
{
  "name": "secure_file_reader",
  "version": "1.0.0",
  "capabilities": ["file_read"],
  "permissions": {
    "network": false,
    "filesystem_read": true,
    "filesystem_write": false,
    "process_exec": false
  },
  "entrypoint": "main.py",
  "description": "Securely reads files from a specific directory"
}
```

```python
#!/usr/bin/env python3
import os
import json
import sys

ALLOWED_DIR = "/safe/read/only/directory"

def validate_path(filepath):
    """Ensure file is within allowed directory."""
    full_path = os.path.abspath(filepath)
    allowed_path = os.path.abspath(ALLOWED_DIR)
    if not full_path.startswith(allowed_path):
        raise ValueError("Access denied: path outside allowed directory")
    return full_path

def main():
    try:
        data = json.load(sys.stdin)
        filepath = data.get("filepath")
        
        if not filepath:
            raise ValueError("filepath is required")
        
        # Validate path before reading
        safe_path = validate_path(filepath)
        
        # Read file
        with open(safe_path, 'r') as f:
            content = f.read()
        
        print(json.dumps({
            "status": "success",
            "content": content
        }))
    except Exception as e:
        print(json.dumps({
            "status": "error",
            "message": str(e)
        }))

if __name__ == "__main__":
    main()
```

## Known Security Considerations

### 1. Network Access

Skills with `network: true` can make arbitrary network requests. Review these skills carefully.

### 2. Process Execution

Skills with `process_exec: true` can run arbitrary commands. Only grant to trusted skills.

### 3. File System Access

Skills with filesystem permissions can access files. Restrict to specific directories when possible.

### 4. Code Synthesis

The Synth module generates code automatically. Review generated code before execution.

## Security Updates

### How to Stay Secure

1. **Subscribe** to security advisories:
   - GitHub releases
   - RSS feed (coming soon)
   - Security mailing list (coming soon)

2. **Monitor** dependency updates:
   ```bash
   cargo update
   cargo audit
   ```

3. **Review** changelog for security updates:
   ```bash
   # Check CHANGELOG.md for security fixes
   ```

### Update Process

```bash
# Update to latest version
git pull origin main
cargo build --release

# Verify installation
cargo run -- --version
```

## Vulnerability Disclosure

### Coordination

We follow responsible disclosure:

1. Report privately
2. Verify and fix
3. Coordinate disclosure
4. Public advisory with fix

### Credits

Security researchers will be credited in:

- Security advisory
- Release notes
- Hall of Fame (coming soon)

### Bug Bounty

We currently don't have a formal bug bounty program. However, significant security discoveries may be eligible for:

- Recognition in project credits
- Skill Router merchandise (coming soon)
- Invitation to contributor program

## Additional Resources

- [Rust Security Guidelines](https://doc.rust-lang.org/nomicon/appendix-07-nightly-rust.html)
- [OWASP Secure Coding Practices](https://owasp.org/www-project-secure-coding-practices-quick-reference-guide/)
- [CWE Top 25](https://cwe.mitre.org/top25/archive/2022/2022_top25_list.html)

## Contact

For security-related questions not involving vulnerabilities:

- **Email**: `security@aionui.org`
- **GitHub Discussions**: [Security Category](https://github.com/aionui/skill-router/discussions/categories/security)

Thank you for helping keep Skill Router secure! 🔒