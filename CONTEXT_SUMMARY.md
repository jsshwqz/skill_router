# Skill Router 项目上下文总结

> **创建日期**: 2026-03-11
> **最后更新**: 2026-03-11
> **版本**: v0.1.0
> **状态**: 已安装 ✓

---

## 项目概述

| 属性 | 值 |
|------|-----|
| 名称 | skill-router |
| 语言 | Rust |
| 平台 | Windows 64-bit |
| GitHub | https://github.com/jsshwqz/skill_router |
| Gitee | https://gitee.com/jsshwqz/skill_router |

---

## 核心功能

通用技能路由器，自动识别任务意图并调用合适的技能：

| 能力 | 说明 |
|------|------|
| YAML 解析 | 解析 YAML 配置文件 |
| JSON 处理 | 解析、分析 JSON 数据 |
| 文本摘要 | 总结文本内容 |
| 网络搜索 | 搜索互联网信息 |
| 技能合成 | 动态生成新技能 |
| 自主编排 | 多步骤任务协调 |

---

## 安装位置

| 组件 | 路径 |
|------|------|
| **可执行文件** | `C:\Users\Administrator\AppData\Roaming\AionUi\aionui\gemini-temp-1772957577810\skill-router\skill-router.exe` |
| **技能定义** | `C:\Users\Administrator\.gemini\skills\skill-router\SKILL.md` |
| **工作目录** | `C:\Users\Administrator\AppData\Roaming\AionUi\aionui\gemini-temp-1772957577810\skill-router\` |

---

## 调用方式

```powershell
# JSON 输出模式（推荐）
& "C:\Users\Administrator\AppData\Roaming\AionUi\aionui\gemini-temp-1772957577810\skill-router\skill-router.exe" --json "任务描述"

# 示例
& "...\skill-router.exe" --json "yaml parse"
& "...\skill-router.exe" --json "web search for python tutorials"
& "...\skill-router.exe" --json "summarize text: ..."
```

**输出格式**:
```json
{"status":"success|error","skill":"skill_name","duration_ms":89.0}
```

---

## 内置技能

| 技能名 | 能力标签 | 描述 |
|--------|----------|------|
| yaml_parser | yaml_parse | YAML 文件解析 |
| google_search | web_search | 网络搜索 |
| synth_jsonparse | json_parse | JSON 解析分析 |
| synth_textsummarize | text_summarize | 文本摘要生成 |
| synth_skillsynthesize | skill_synthesize | 动态技能合成 |
| autonomous_orchestrator | universal_automation | 自主编排执行 |

---

## 项目结构

```
skill-router/
├── skill-router.exe    # 主程序
├── config.json         # 配置文件
├── registry.json       # 技能注册表
├── skills/             # 技能脚本目录
│   ├── yaml_parser/
│   ├── google_search/
│   ├── synth_jsonparse/
│   ├── synth_textsummarize/
│   ├── synth_skillsynthesize/
│   └── autonomous_orchestrator/
└── logs/               # 日志目录
```

---

## 验证状态

- [x] YAML 解析 ✓
- [x] 文本摘要 ✓
- [x] 网络搜索 ✓
- [x] Gemini 全局技能安装 ✓

---

## 注意事项

1. **路径含空格** - 调用时需用引号包裹
2. **网络要求** - 网络搜索功能需要网络连接
3. **生效方式** - 重启 Gemini CLI 或新开会话后技能生效
4. **JSON 模式** - 推荐使用 `--json` 参数获取结构化输出

---

## 更新日志

| 日期 | 内容 |
|------|------|
| 2026-03-11 | 初始安装，配置全局技能 |

---

*此文件供 AI 模型读取项目上下文，无需重新分析完整代码库。*