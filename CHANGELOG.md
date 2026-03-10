# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-03-10

### Added

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
- yaml_parser skill
- google_search skill
- synth_jsonparse skill
- synth_textsummarize skill
- synth_skillsynthesize skill
- autonomous_orchestrator skill

### Changed

- Replaced Python-based online search with pure Rust reqwest implementation
- Implemented four-phase pipeline: Local Match → SkillsFinder → GitHub Search → Synthesis

### Security

- Integrated security audit and permission validation
- Default deny permission model
- Runtime validation before every execution

[Unreleased]: https://github.com/aionui/skill-router/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/aionui/skill-router/releases/tag/v0.1.0