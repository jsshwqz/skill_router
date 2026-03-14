# Encoding Checker Skill (encoding_checker)

## 📖 Description

The encoding checker skill scans the project for encoding issues to prevent GBK/ANSI encoding problems in source files.

## 🎯 Capabilities

| Capability | Description |
|------------|-------------|
| `encoding_validation` | Validate file encodings |
| `code_quality_check` | Check code quality standards |
| `prevention` | Prevent encoding issues before they occur |

## 🚀 Usage

### 1. Scan Project

```bash
cargo run --release -- scan
```

Or with JSON input:

```json
{
  "action": "scan",
  "project_root": ".",
  "strict_mode": false
}
```

### 2. Check Specific File

```bash
cargo run --release -- check
```

JSON input:

```json
{
  "action": "check",
  "file_path": "skills/context_manager/src/main.rs"
}
```

### 3. Generate Report

```bash
cargo run --release -- report
```

## 📊 Output Format

```json
{
  "status": "success|warning|error",
  "skill": "encoding_checker",
  "file": "File path (optional)",
  "encoding": "UTF-8 or detected encoding",
  "issues": [
    {
      "file": "path",
      "issue": "Description",
      "severity": "warning|error"
    }
  ],
  "report": "Full report (optional)",
  "files_scanned": 10,
  "files_with_issues": 0,
  "error": "Error message (optional)",
  "duration_ms": 45
}
```

## 📁 Project Structure

```
encoding_checker/
├── src/
│   └── main.rs      # Rust main program
├── skill.json       # Skill definition
├── Cargo.toml       # Rust dependencies
└── SKILL.md         # Skill documentation
```

## 🔧 Workflow

```
1. User requests encoding check
   ↓
2. encoding_checker scans files
   ↓
3. Check for GBK/ANSI indicators
   ↓
4. Return structured JSON report
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
  "skill": "encoding_checker",
  "files_scanned": 12,
  "files_with_issues": 0,
  "issues": [],
  "duration_ms": 5
}
```

### Check File
```bash
cargo run --release -- check
```

### Generate Report
```bash
cargo run --release -- report
```

## ⚠️ Common Issues

1. **Chinese characters in source files**: Should use English identifiers
2. **Non-UTF-8 files**: All files should be UTF-8 encoded
3. **Mixed encodings**: Inconsistent encoding across files

## 🛡️ Prevention Rules

1. All source code files (.rs, .py) must be UTF-8 encoded
2. All documentation files (.md) must be UTF-8 encoded
3. Use English identifiers in source code
4. Check encoding before committing
