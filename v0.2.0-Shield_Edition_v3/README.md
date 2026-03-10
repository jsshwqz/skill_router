# Skill Router Shield Edition v4

高性能技能路由器，基于 Rust 构建的智能任务编排系统。

## 新增增强功能 (v4)

### 1. Smart Planner 智能规划器
- **多模式意图识别**: 使用正则匹配 + 关键词推断能力需求
- **任务分解**: 自动将复杂任务拆分为子任务
- **执行策略**: 支持 Sequential/Parallel/Pipeline/Adaptive 四种模式
- **安全防火墙**: 阻止恶意意图注入

### 2. Parallel Executor 并行执行器
- **并行执行**: 支持多技能并行调用
- **Pipeline 模式**: 按依赖关系自动排序执行
- **自适应模式**: 根据任务特征自动选择策略

### 3. Skill Cache 技能缓存
- **LRU 缓存**: 高效的能力匹配缓存
- **TTL 过期**: 自动清理过期缓存
- **预热机制**: Registry 加载时自动预热缓存

### 4. Retry Engine 重试引擎
- **多策略重试**: Fixed/Exponential/Immediate
- **熔断器**: Circuit Breaker 防止级联失败
- **回退链**: 支持备选技能自动切换

### 5. Chain Orchestrator 链式编排
- **技能链**: 多技能串联执行
- **条件执行**: 支持条件判断
- **失败策略**: Stop/Skip/Retry/Fallback

### 6. Metrics 指标监控
- **Prometheus 兼容**: 导出标准格式指标
- **技能级统计**: 每个技能的执行统计
- **实时汇总**: 任务成功/失败率、延迟等

### 7. 增强安全审计
- **模式匹配**: 20+ 危险模式检测
- **网络检测**: 识别外部 URL 和网络请求
- **权限合规**: 自动检查代码与权限一致性

## 目录结构

```
src/
├── main.rs              # 主入口
├── lib.rs               # 库导出
├── models.rs            # 数据模型
├── planner.rs           # 基础规划器
├── matcher.rs           # 技能匹配
├── executor.rs          # 执行器
├── security.rs          # 权限验证
├── security_analyzer.rs # 安全审计 (增强)
├── loader.rs            # 技能加载
├── registry.rs          # 注册表管理
├── lifecycle.rs         # 生命周期
├── online_search.rs     # 在线搜索
├── synth.rs             # 技能合成
└── enhanced/            # 增强模块
    ├── mod.rs
    ├── smart_planner.rs
    ├── parallel_executor.rs
    ├── skill_cache.rs
    ├── retry_engine.rs
    ├── chain_orchestrator.rs
    └── metrics.rs
```

## 使用方式

```bash
# 基础用法
skill-router "parse this yaml file"

# JSON 输出
skill-router --json "search for python tutorials"

# 指定配置
skill-router --config custom.json "complex task"
```

## 配置示例

```json
{
  "enable_auto_install": true,
  "skills_dir": "skills",
  "registry_file": "registry.json",
  "logs_dir": "logs",
  "trusted_sources": ["https://github.com/trusted"],
  "llm_enabled": true,
  "llm_endpoint": "https://api.example.com/v1",
  "max_retries": 3,
  "parallel_workers": 4,
  "cache_ttl_seconds": 3600
}
```

## 编译

```bash
cargo build --release
```

---
*Version 4.0.0 - Shield Edition*