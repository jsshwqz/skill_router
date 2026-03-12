# Skill Router v0.1.0 - Windows 64位版本

## 快速开始

### 1. 双击运行
直接双击 skill-router.exe 即可启动交互模式。

### 2. 命令行使用
```powershell
# 基本用法
.\skill-router.exe "解析这个yaml文件"

# JSON输出模式（用于AI集成）
.\skill-router.exe --json "搜索天气信息"

# 查看帮助
.\skill-router.exe --help
```

### 3. 示例任务
```powershell
.\skill-router.exe "解析YAML配置"
.\skill-router.exe "搜索Python教程"
.\skill-router.exe "总结这段文本内容"
```

## 目录结构

```
skill-router/
├── skill-router.exe    # 主程序
├── config.json         # 配置文件
├── registry.json       # 技能注册表
├── skills/             # 技能目录
│   ├── yaml_parser/    # YAML解析技能
│   ├── google_search/  # 搜索技能
│   └── ...
├── logs/               # 日志目录
└── README.txt          # 本文件
```

## 内置技能

| 技能名 | 功能 |
|--------|------|
| yaml_parser | YAML文件解析 |
| google_search | 网络搜索 |
| synth_jsonparse | JSON解析 |
| synth_textsummarize | 文本摘要 |
| synth_skillsynthesize | 技能合成 |
| autonomous_orchestrator | 自主编排 |

## 配置说明

编辑 config.json 可修改：
- enable_auto_install: 是否自动安装新技能
- skills_dir: 技能目录路径
- trusted_sources: 信任的技能来源

## 系统要求

- Windows 10/11 64位
- 网络连接（用于在线搜索功能）

## 问题反馈

- GitHub: https://github.com/jsshwqz/skill_router/issues
- Gitee: https://gitee.com/jsshwqz/skill_router/issues

## 许可证

MIT License - 详见 LICENSE 文件