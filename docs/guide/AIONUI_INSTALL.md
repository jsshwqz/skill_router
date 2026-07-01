# 🚀 AionUI 技能集成指南

要把 Aion Forge 集成到 **AionUI** 中，有两种方式：

---

## 方式 A：ACP 扩展自动识别（推荐）— 2026-05-27 更新

Aion Forge 作为 AionUI 的 **ACP 扩展**，通过扩展系统自动识别，无需手动添加。

### 前置条件
- AionUI v2.1.3+
- Aion Forge v0.7.0+（包含 `aion-forge-cli.exe` 和 `aionext-forge/` 扩展目录）

### 部署步骤

1. **编译 ACP 服务器**
   ```bash
   cargo build --release -p aion-forge-cli
   ```

2. **确保扩展文件完整**
   扩展目录 `aionext-forge/` 必须包含：
   ```
   aionext-forge/
   ├── aion-extension.json     # engines.aionui: "^2.0.0"
   ├── assets/
   │   └── forge-icon.svg
   └── contributes/
       ├── acp-adapters.json
       ├── assistants.json
       └── assistants/
           └── forge-context.md
   ```

3. **配置 API Key**
   在与 `aion-forge-cli.exe` 同级的 `.env` 文件中：
   ```
   AI_BASE_URL=https://openrouter.ai/api/v1
   AI_API_KEY=sk-or-v1-xxx
   AI_MODEL=inclusionai/ring-2.6-1t
   ```

4. **将扩展部署到 AionUI**
   将 `aionext-forge/` 整个目录复制到 AionUI 用户数据目录下的 `extensions/` 中。
   常见位置：`C:\Users\{用户名}\AppData\Local\AionUI\extensions\aionext-forge\`

5. **重启 AionUI**
   扩展应在启动时自动识别，在助手列表中可见。

6. **在 AionUI 中配置 AI 引擎**
   若 AI 引擎显示"无大模型可用"，需在 AionUI 设置中配置 API Key，或在 `.env` 中设置。

### 自动识别机制
- AionUI 启动时 aioncore 扫描 `extensions/` 目录
- 读取每个子目录的 `aion-extension.json`
- 检查 `engines.aionui` semver 范围是否匹配 AionUI 版本（v2.1.3 匹配 `^2.0.0`）
- 合并 `contributes.acpAdapters` 和 `contributes.assistants`
- 通过 `/api/extensions/acp-adapters` 和 `/api/assistants` 暴露给 UI

---

## 方式 B：作为 MCP 技能（传统方式）

通过 AionUI 的技能系统手动添加。

1. 打开 **AionUI** 客户端
2. 找到 **"技能管理" (Skills)** 或 **"设置 → 插件/技能"**
3. 点击 **"添加本地技能" (Add Local Skill)**
4. 选择编译后的 `aion-forge-cli.exe` 所在目录
5. AionUI 会自动识别 `skill.json` 中定义的能力

---

## 配置说明

### ACP 适配器配置 (`aionext-forge/contributes/acp-adapters.json`)
```json
{
  "id": "aion-forge",
  "name": "Aion Forge",
  "connectionType": "stdio",
  "cliCommand": "aion-forge-cli.exe",
  "acpArgs": ["acp"],
  "defaultCliPath": "D:\\test\\aionui\\forge\\aion-forge-cli.exe",
  "authRequired": false
}
```

### 助手配置 (`aionext-forge/contributes/assistants.json`)
```json
{
  "id": "aion-forge-assistant",
  "name": "Aion Forge",
  "presetAgentType": "aion-forge",
  "avatar": "assets/forge-icon.svg",
  "contextFile": "contributes/assistants/forge-context.md"
}
```

---

## 💡 排错提示

| 现象 | 可能原因 | 解决方法 |
|------|---------|---------|
| 扩展未自动识别 | `engines.aionui` 版本不匹配 | 确认版本范围含 `^2.0.0` |
| 扩展未自动识别 | 扩展未部署到正确路径 | 确认 `extensions/aionext-forge/` 在 AionUI userData 目录下 |
| "无大模型可用" | 缺少 API Key | 在 `.env` 或 AionUI 设置中配置 API Key |
| ACP 连接失败 | `cliCommand` 找不到可执行文件 | 使用绝对路径 `defaultCliPath` |