# Aion Skill Router Workspace

A high-performance, modular AI Agent Capability Router & Execution Engine built in Rust.

## Architecture

This workspace is divided into four specialized crates:

- **aion-types**: Core data structures and shared traits.
- **aion-memory**: Long-term memory management and distillation.
- **aion-intel**: AI-driven planning, synthesis, and discovery.
- **aion-router**: The core execution engine and facade.
- **aion-cli**: Standard command-line interface for universal use.

## Installation

### For CLI Users
```bash
git clone https://github.com/aioncore/aion-core.git
cd aion-core
cargo install --path aion-cli
```

### For Rust Developers
Add this to your `Cargo.toml`:
```toml
[dependencies]
aion-router = { git = "https://github.com/aioncore/aion-core.git", package = "aion-router" }
```

## Usage

```bash
aion-cli "Perform a web search for the latest Rust release notes"
```

## License
MIT
