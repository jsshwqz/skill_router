# 智能搜索系统 - 语言触发版

## 🎯 第一阶段功能

**无感使用方式**: 用户只需输入 `搜索：关键词` 或 `search:keyword` 即可触发智能搜索

## 🔧 使用方法

### 1. 启动搜索系统
```bash
python smart_search_v1.py
```

### 2. 触发搜索
- **中文模式**: 输入 `搜索：https://example.com`
- **英文模式**: 输入 `search:Rust programming`

### 3. 退出程序
输入 `quit` 或按 `Ctrl+C`

## 🌟 核心特性

✅ **语言触发**: 无需复杂命令，自然语言前缀即可  
✅ **智能路由**: 自动选择最佳搜索源（Jina Reader/Exa Search/浏览器自动化）  
✅ **透明决策**: 显示详细的决策过程和置信度  
✅ **零成本**: 完全免费，无需API密钥  
✅ **完全降级**: 无浏览器环境下也能正常工作  

## 📊 搜索源支持

1. **Jina Reader**: 静态网页内容提取
2. **Exa Search**: 语义搜索和通用查询  
3. **浏览器自动化**: 动态网页渲染（条件性启用）

## 🚀 示例

```
> search:https://example.com
Searching: https://example.com
Success!
Title: Example Domain

URL Source: https://example.com/

Published Time: Wed, 11 Mar 2026 19:06:45 GMT

Warning: This is a cached snapshot of the original page, consider retry with caching opt-out.

Markdown Content:
This domain is for use in documentation examples without needing permission. Avoid use in operations.

[Learn more](https://iana.org/domains/example)
```

## 📋 下一步计划

- **第二阶段**: 系统托盘集成 + 全局快捷键
- **第三阶段**: 剪贴板监控自动触发
- **第四阶段**: 浏览器扩展集成

---
**当前状态**: ✅ 第一阶段完成，支持语言触发搜索