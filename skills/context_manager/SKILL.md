# Context Manager Skill (context_manager)

## 📖 Description

The context manager skill manages project context, including reading files, generating summaries, scanning project structure, and updating CONTEXT.md.

## 🎯 Capabilities

| Capability | Description |
|------------|-------------|
| `context_management` | Project context management |
| `file_scanning` | Scan project files |
| `summary_generation` | Generate text summaries |
| `context_update` | Update CONTEXT.md |

## 🚀 Usage

### 1. Scan Project

```bash
cargo run --release -- scan
```

Or with JSON input:

```json
{
  "action": "scan"
}
```

### 2. Read File

```bash
cargo run --release -- read
```

JSON input:

```json
{
  "action": "read",
  "file_path": "README.md"
}
```

### 3. Generate Summary

```bash
cargo run --release -- summarize
```

JSON input:

```json
{
  "action": "summarize",
  "file_path": "long_document.md",
  "max_summary_length": 500
}
```

### 4. Update CONTEXT.md

```bash
cargo run --release -- update
```

JSON input:

```json
{
  "action": "update",
  "file_path": "Latest update content"
}
```

## 📊 Output Format

```json
{
  "status": "success|error",
  "skill": "context_manager",
  "content": "File content (optional)",
  "summary": "Summary content (optional)",
  "files_scanned": 10,
  "updated_file": "CONTEXT.md",
  "error": "Error message (optional)",
  "duration_ms": 45
}
```

## 📁 Project Structure

```
context_manager/
├── src/
│   └── main.rs      # Rust main program
├── skill.json       # Skill definition
├── Cargo.toml       # Rust dependencies
└── SKILL.md         # Skill documentation
```

## 🔧 Workflow

```
1. User requests context operation
   ↓
2. context_manager executes corresponding action
   ↓
3. Read/Generate/Update file
   ↓
4. Return structured JSON result
```

## 📝 Examples

### Scan Project
```bash
cargo run --release -- scan
```

Output:
```json
{
  "status": "success",
  "skill": "context_manager",
  "content": "# Project Scan Summary\n...",
  "summary": "# Project Scan Summary\n...",
  "files_scanned": 12,
  "duration_ms": 2
}
```

### Read File
```bash
cargo run --release -- read
```

### Update Context
```bash
cargo run --release -- update
```
