# 🚀 AionUI 技能集成指南

要把这个基于 Rust 的强大引擎 (`aion_forge`) 集成到你的 **AionUI** Agent 中，请遵循以下步骤。

---

## 1. 准备工作 (编译引擎)
在 AionUI 运行这些技能之前，建议先在本地编译好，以确保运行速度。

1. 打开终端，进入项目根目录。
2. 运行编译命令：
   ```bash
   cargo build --release
   ```

---

## 2. 在 AionUI 中安装
AionUI 通过读取根目录下的 `skill.json` 来识别技能。

### 方法 A：通过目录挂载 (推荐)
1. 打开 **AionUI** 客户端。
2. 找到 **“技能管理” (Skills)** 或 **“设置 -> 插件/技能”**。
3. 点击 **“添加本地技能” (Add Local Skill)**。
4. 选择你解压后的目录。
5. AionUI 会自动识别出 `skill.json` 中定义的 `complex_automation` 等能力。

### 方法 B：手动移动到技能文件夹
1. 找到 AionUI 的安装目录下的 `skills/` 文件夹。
2. 将整个 `aion_forge` 文件夹复制进去。
3. 重启 AionUI。

---

## 3. 配置文件说明 (`skill.json`)
你的 `skill.json` 已经配置好了基本权限与 **AI-Native Schema (参数强校验)**：
- **entrypoint**: `cargo run -p aion-cli --` (这会自动调用 Rust 编译器运行，如果你想更快，可以改为编译后的路径 `target/release/aion-cli`)。
- **permissions**: 已开启网络、文件读写及进程执行权限，确保“级联搜索”和“代码生成”能正常工作。
- **api_schema**: 自动集成了 `manifest.json` 中 25 种核心能力的 OpenAPI 格式输入定义 (如 `code_generate`, `complex_automation`)，供 AionUI 或大模型精准识别参数格式，最大程度杜绝“大模型幻觉”。

---

## 4. 如何开始使用？
一旦安装成功，你可以在 AionUI 的对话框中直接下达指令，例如：

> *"帮我分析这个项目的代码并生成一个单元测试"*

AionUI 会自动路由到此技能，并调用 `aion-cli` 完成任务。你可以通过查看 AionUI 的运行日志来确认它是否正确调用了 `aion_forge` 引擎。

---

## 💡 小贴士
- **环境变量**: 如果你使用了级联搜索功能，请确保系统环境变量中配置了 `SERPAPI_KEY`。
- **性能**: 第一次运行 `cargo run` 会比较慢（因为需要编译），之后的运行会非常快。
