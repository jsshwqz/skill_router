# 安装指南

本指南帮助 AI 助手自动安装和配置 Skill Router 项目。

## 快速安装

### 前置要求
- Rust 1.70 或更高版本
- Python 3.8+（用于技能执行）
- Git（可选，用于从 GitHub 克隆）

### 安装步骤

#### 方法 1: Git Clone（推荐）
```bash
# 克隆仓库
git clone https://github.com/aionui/skill-router.git
cd skill-router

# 构建项目
cargo build --release

# 验证安装
cargo run --release -- --version
```

#### 方法 2: 下载 Release 压缩包
```bash
# 下载并解压 v0.0.1.zip
# 进入项目目录
cd skill-router

# 构建项目
cargo build --release

# 验证安装
cargo run --release -- --version
```

## AI 使用方式

### 挂载目录
```
/workspace/skill_router
```

### 构建命令
```bash
cargo build --release
```

### 可执行文件位置
```
target/release/skill-router.exe  (Windows)
target/release/skill-router      (Linux/macOS)
```

### 运行示例
```bash
# 基本用法
cargo run --release -- "parse this yaml file"

# JSON 输出（适合 AI 解析）
cargo run --release -- --json "search for weather information"
```

## 配置文件

- `config.json` - 主配置文件
- `registry.json` - 技能注册表
- `skills/` - 技能目录

## 技能列表

| 技能 | 版本 | 说明 |
|------|------|------|
| yaml_parser | v0.0.1 | YAML 解析技能 |
| google_search | v0.0.1 | 网络搜索技能 |
| synth_jsonparse | v0.0.1 | JSON 解析合成技能 |
| synth_textsummarize | v0.0.1 | 文本摘要合成技能 |
| synth_skillsynthesize | v0.0.1 | 技能合成技能 |
| autonomous_orchestrator | v0.0.1 | 任务编排技能 |

## 故障排除

### 构建失败
```bash
# 更新 Rust
rustup update

# 清理并重新构建
cargo clean
cargo build --release
```

### 技能执行失败
检查 Python 环境和依赖：
```bash
python --version  # 确认 Python 3.8+
pip list  # 检查依赖
```

## 版本信息

- 项目版本: v0.0.1
- 技能版本: v0.0.1

## 下一步

查看完整文档：
- `README.md` - 项目概述
- `VERSION.md` - 版本历史
- `CHANGELOG.md` - 变更日志
