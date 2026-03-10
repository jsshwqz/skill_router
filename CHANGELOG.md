# Changelog

All notable changes to Skill Router will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.0.1] - 2026-03-10 - Initial Release

### ✨ Core Features
- **Planner**: Task intent parsing and capability inference
- **Loader**: Dynamic skill metadata loading
- **Registry**: Skill state persistence management
- **Matcher**: Capability-based skill matching algorithm
- **SkillsFinder**: Intelligent skill discovery with scoring
- **OnlineSearch**: Pure Rust GitHub API search with security audit
- **Synth**: Automatic skill code synthesis (Rust-first)
- **Executor**: Secure process execution
- **Security**: Strict permission validation model
- **Lifecycle**: Automatic skill lifecycle management

### 🏗️ Architecture
- Rust-first native implementation
- reqwest HTTP client for online search
- Four-phase Pipeline: Local Match → SkillsFinder → GitHub Search → Synthesis
- Integrated security audit and permission validation

### 📦 Implemented Skills
- yaml_parser (v0.0.1)
- google_search (v0.0.1)
- synth_jsonparse (v0.0.1)
- synth_textsummarize (v0.0.1)
- synth_skillsynthesize (v0.0.1)
- autonomous_orchestrator (v0.0.1)

### 📚 Documentation
- README.md - Comprehensive project documentation
- CONTRIBUTING.md - Contributor guidelines
- SECURITY.md - Security policy
- LICENSE - MIT License