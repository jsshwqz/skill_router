<div align="center">

# Skill Router

[![Rust](https://img.shields.io/badge/Rust-2021-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Version](https://img.shields.io/badge/version-0.1.0-blue.svg)](https://github.com/aionui/skill-router)
[![Build Status](https://img.shields.io/badge/build-passing-green.svg)]()

**Rust-native autonomous skill discovery and execution system**

[Features](#-features) • [Quick Start](#-quick-start) • [Documentation](#-documentation) • [Contributing](#-contributing)

</div>

## 📖 Overview

Skill Router is a sophisticated, Rust-first system for automatically discovering, matching, and executing software capabilities. It intelligently routes tasks to appropriate skills through a multi-phase pipeline:

1. **Local Matching** - Finds existing skills in your registry
2. **SkillsFinder** - Discovers related skills using intelligent algorithms
3. **Online Search** - Searches GitHub for missing capabilities
4. **Synthesis** - Automatically generates new skills when needed

Built with security, performance, and extensibility in mind, Skill Router provides a robust foundation for autonomous agent workflows.

## ✨ Features

- 🎯 **Intelligent Skill Matching** - Advanced capability-based matching algorithm
- 🔍 **Multi-Phase Discovery** - Local → Intelligent → GitHub → Synthesis pipeline
- 🔒 **Security-First** - Strict permission validation and audit logging
- 📊 **Usage Analytics** - Built-in performance tracking and lifecycle management
- 🚀 **Pure Rust** - High performance with minimal external dependencies
- 🔌 **Extensible** - Easy skill development and integration
- 🤖 **Agent-Ready** - JSON output mode for seamless AI/LLM integration

## 🚀 Quick Start

### Prerequisites

- Rust 1.70 or higher
- Python 3.8+ (for skill execution)
- Git (for GitHub skill discovery)

### Installation

```bash
# Clone the repository
git clone https://github.com/aionui/skill-router.git
cd skill-router

# Build the project
cargo build --release

# The binary will be available at target/release/skill-router
```

### Usage

```bash
# Basic usage
cargo run --release -- "parse this yaml file"

# JSON output (for AI agents)
cargo run --release -- --json "search for weather information"

# Custom configuration
cargo run --release -- --config custom-config.json "task description"
```

### Development Mode

```bash
# Run tests
cargo test

# Check code
cargo check

# Format code
cargo fmt

# Run with debug output
cargo run -- "task description"
```

## 📚 Documentation

### Core Modules

| Module | Description |
|--------|-------------|
| [`Planner`](src/planner.rs) | Task intent parsing and capability inference |
| [`Loader`](src/loader.rs) | Dynamic skill metadata loading |
| [`Registry`](src/registry.rs) | Skill state persistence and management |
| [`Matcher`](src/matcher.rs) | Capability-based skill matching |
| [`SkillsFinder`](src/skills_finder.rs) | Intelligent skill discovery |
| [`OnlineSearch`](src/online_search.rs) | GitHub API search and installation |
| [`Synth`](src/synth.rs) | Automatic skill code synthesis |
| [`Executor`](src/executor.rs) | Secure process execution |
| [`Security`](src/security.rs) | Permission validation and audit |
| [`Lifecycle`](src/lifecycle.rs) | Automated skill lifecycle management |

### Configuration

Create a `config.json` in your project root:

```json
{
  "enable_auto_install": false,
  "skills_dir": "skills",
  "registry_file": "registry.json",
  "logs_dir": "logs",
  "trusted_sources": [
    "https://github.com/trusted-source"
  ],
  "llm_enabled": false,
  "llm_command": null
}
```

### Skill Development

Create a new skill in the `skills/` directory:

```json
{
  "name": "my_skill",
  "version": "1.0.0",
  "capabilities": ["my_capability"],
  "permissions": {
    "network": false,
    "filesystem_read": true,
    "filesystem_write": false,
    "process_exec": false
  },
  "entrypoint": "main.py",
  "description": "Description of what this skill does"
}
```

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for detailed skill development guidelines.

## 🔒 Security

Skill Router implements defense-in-depth security:

- **Default Deny** - All permissions default to false
- **Explicit Authorization** - Skills must declare required permissions
- **Runtime Validation** - Security checks before every execution
- **Audit Logging** - Comprehensive execution and security event logging
- **Repository Scanning** - Automated security analysis of downloaded skills

See [`SECURITY.md`](SECURITY.md) for detailed security information.

## 📊 Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        User Task                            │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│  Planner - Intent Parsing & Capability Inference            │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│  Phase 1: Matcher - Local Skill Registry                   │
│  ┌─────────────────┬─────────────────┬─────────────────┐   │
│  │ YAML Parser     │ JSON Parser     │ Google Search   │   │
│  └─────────────────┴─────────────────┴─────────────────┘   │
└────────────────────────┬────────────────────────────────────┘
                         │ (No match)
                         ▼
┌─────────────────────────────────────────────────────────────┐
│  Phase 2: SkillsFinder - Intelligent Discovery              │
│  • Related skill matching in registry                       │
│  • google_search skill integration                          │
│  • Scoring and recommendation algorithm                     │
└────────────────────────┬────────────────────────────────────┘
                         │ (No skills found)
                         ▼
┌─────────────────────────────────────────────────────────────┐
│  Phase 3: OnlineSearch - GitHub API                        │
│  • Repository search and validation                         │
│  • Automatic installation with security audit               │
│  • Skill metadata verification                              │
└────────────────────────┬────────────────────────────────────┘
                         │ (No results)
                         ▼
┌─────────────────────────────────────────────────────────────┐
│  Phase 4: Synth - Code Generation                           │
│  • Rust-first code synthesis                                │
│  • Automatic skill creation                                 │
│  • Integration with existing pipeline                       │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│  Executor - Secure Execution                               │
│  • Permission validation                                    │
│  • Process isolation                                        │
│  • Performance tracking                                     │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│  Registry & Lifecycle - State Management                    │
│  • Usage statistics                                         │
│  • Performance metrics                                      │
│  • Lifecycle recommendations                                 │
└─────────────────────────────────────────────────────────────┘
```

## 🤝 Contributing

We welcome contributions! Please see [`CONTRIBUTING.md`](CONTRIBUTING.md) for guidelines on:

- Setting up the development environment
- Coding standards and conventions
- Submitting pull requests
- Reporting issues
- Developing new skills

## 📝 License

This project is licensed under the MIT License - see the [`LICENSE`](LICENSE) file for details.

## 🙏 Acknowledgments

- Built with [Rust](https://www.rust-lang.org/)
- Inspired by modern agent architecture patterns
- Powered by the amazing open-source community

## 📞 Support

- 📖 [Documentation](https://github.com/aionui/skill-router/wiki)
- 🐛 [Issue Tracker](https://github.com/aionui/skill-router/issues)
- 💬 [Discussions](https://github.com/aionui/skill-router/discussions)

## 🗺️ Roadmap

- [ ] LLM-powered task decomposition
- [ ] MCP (Model Context Protocol) support
- [ ] Enhanced skill marketplace
- [ ] Docker containerization
- [ ] WASM-based skill execution
- [ ] Web-based management UI

---

<div align="center">

**Made with ❤️ by the AionUi Team**

[⬆ Back to Top](#skill-router)

</div>