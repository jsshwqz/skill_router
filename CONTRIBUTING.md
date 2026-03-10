# Contributing to Skill Router

Thank you for your interest in contributing to Skill Router! We welcome contributions from everyone.

## 📋 Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Workflow](#development-workflow)
- [Coding Standards](#coding-standards)
- [Skill Development](#skill-development)
- [Testing](#testing)
- [Submitting Changes](#submitting-changes)
- [Reporting Issues](#reporting-issues)

## 🤝 Code of Conduct

Be respectful, inclusive, and constructive. We expect all contributors to adhere to our Code of Conduct (see [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)).

## 🚀 Getting Started

### Prerequisites

- Rust 1.70 or higher
- Python 3.8+ (for skill execution)
- Git

### Setting Up Development Environment

```bash
# Fork and clone the repository
git clone https://github.com/your-username/skill-router.git
cd skill-router

# Add upstream remote
git remote add upstream https://github.com/aionui/skill-router.git

# Create a new branch
git checkout -b feature/your-feature-name

# Install development dependencies
cargo install cargo-watch cargo-edit
```

### Building

```bash
# Development build
cargo build

# Release build
cargo build --release

# Run with cargo watch for auto-rebuild
cargo watch -x check -x test -x run
```

## 🔄 Development Workflow

1. **Fork** the repository
2. **Create a branch** for your feature or bugfix
3. **Make your changes** following our coding standards
4. **Test** your changes thoroughly
5. **Commit** with clear, descriptive messages
6. **Push** to your fork
7. **Submit** a pull request

### Commit Message Convention

We follow conventional commits:

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `perf`: Performance improvements
- `test`: Test additions or changes
- `chore`: Build process or auxiliary tool changes

Examples:
```
feat(skills): add capability to search by keywords
fix(online-search): handle rate limiting from GitHub API
docs(readme): update installation instructions
```

## 📐 Coding Standards

### Rust Code Style

- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` for formatting
- Run `cargo clippy` for linting
- Prefer explicit error handling with `Result` and `Option`

```bash
# Format code
cargo fmt

# Run linter
cargo clippy -- -D warnings

# Check for unused dependencies
cargo +nightly udeps
```

### Documentation

- Document all public APIs with `///`
- Include examples in documentation
- Keep documentation up to date with code changes

```rust
/// Performs intelligent skill discovery based on required capabilities.
///
/// # Arguments
///
/// * `registry` - The current skill registry
/// * `config` - System configuration
/// * `required_caps` - List of required capabilities
/// * `task` - The task description
///
/// # Returns
///
/// `Some(Vec<SkillMetadata>)` if skills are found, `None` otherwise
///
/// # Examples
///
/// ```no_run
/// let skills = SkillsFinder::discover_skills(&registry, &config, &caps, &task);
/// ```
pub fn discover_skills(/* ... */) -> Option<Vec<SkillMetadata>> {
    // ...
}
```

## 🧩 Skill Development

### Creating a New Skill

1. Create a directory in `skills/`:

```bash
mkdir -p skills/my_awesome_skill
```

2. Create `skill.json`:

```json
{
  "name": "my_awesome_skill",
  "version": "1.0.0",
  "capabilities": [
    "awesome_capability"
  ],
  "permissions": {
    "network": true,
    "filesystem_read": false,
    "filesystem_write": false,
    "process_exec": false
  },
  "entrypoint": "main.py",
  "description": "An awesome skill that does amazing things"
}
```

3. Implement the skill logic (`main.py`):

```python
#!/usr/bin/env python3
import sys
import json

def main():
    """Main entry point for the skill."""
    # Read input from stdin
    input_data = json.load(sys.stdin)
    
    # Process the input
    result = {
        "status": "success",
        "data": "Your processing result here"
    }
    
    # Output result
    print(json.dumps(result))

if __name__ == "__main__":
    main()
```

4. Test the skill:

```bash
cargo run -- "use my awesome capability"
```

### Skill Guidelines

- **Security First**: Minimize permissions, validate all inputs
- **Idempotency**: Skills should be safe to run multiple times
- **Error Handling**: Return clear error messages
- **Performance**: Optimize for speed and resource usage
- **Testing**: Include tests for skill functionality

## 🧪 Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_name

# Run tests in release mode
cargo test --release
```

### Writing Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_matching() {
        let registry = create_test_registry();
        let caps = vec!["yaml_parse".to_string()];
        let skill = Matcher::find_best_match(&registry, &caps);
        assert!(skill.is_some());
        assert_eq!(skill.unwrap().name, "yaml_parser");
    }

    #[test]
    fn test_security_validation() {
        let skill = create_test_skill();
        let result = Security::validate_permissions(&skill, &["network"]);
        assert!(result.is_err());
    }
}
```

### Integration Tests

Add integration tests in the `tests/` directory:

```bash
tests/
├── integration/
│   ├── test_pipeline.rs
│   ├── test_online_search.rs
│   └── test_skill_execution.rs
```

## 📤 Submitting Changes

### Pull Request Checklist

Before submitting a PR, ensure:

- [ ] Code follows style guidelines (`cargo fmt`, `cargo clippy`)
- [ ] All tests pass (`cargo test`)
- [ ] Documentation is updated
- [ ] Commit messages are clear and follow conventions
- [ ] PR description explains the changes
- [ ] No merge conflicts with upstream

### Pull Request Template

Use the PR template in `.github/pull_request_template.md`:

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Testing
How did you test these changes?

## Checklist
- [ ] Tests pass
- [ ] Documentation updated
- [ ] Code follows guidelines
```

## 🐛 Reporting Issues

When reporting bugs or requesting features:

1. Search existing issues first
2. Use the issue template
3. Provide clear, reproducible steps
4. Include environment information

### Bug Report Template

```markdown
## Description
Clear description of the bug

## Steps to Reproduce
1. Step one
2. Step two
3. Step three

## Expected Behavior
What should happen

## Actual Behavior
What actually happened

## Environment
- OS: [e.g., Windows 10, macOS 12]
- Rust version: [e.g., 1.70.0]
- Skill Router version: [e.g., 0.0.1]

## Logs
Relevant log output
```

## 💡 Feature Requests

For feature requests:

1. Explain the use case clearly
2. Provide examples of how it would be used
3. Discuss potential implementation approaches
4. Consider offering to implement it

## 📞 Getting Help

- Check existing [documentation](https://github.com/aionui/skill-router/wiki)
- Search [issues](https://github.com/aionui/skill-router/issues)
- Start a [discussion](https://github.com/aionui/skill-router/discussions)
- Join our community chat (if available)

## 🎉 Recognition

Contributors will be acknowledged in:
- README.md contributors section
- Release notes for significant contributions
- Project documentation

Thank you for contributing to Skill Router! 🙏