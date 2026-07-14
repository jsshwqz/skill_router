# 研发助手工具链架构

## 目标

在 Aion Forge 内部构建一套纯净的模型研发辅助工具，核心能力：

1. **模型谱分析** — 权重矩阵的特征值分解、奇异值分布、主成分分析
2. **迭代精炼引擎** — 探测→分析→修正→验证的闭环优化
3. **质量评估流水线** — 困惑度、KL散度、连贯性评分等多维指标
4. **自动化超参搜索** — 基于 Optuna 风格的贝叶斯优化

## 新增 crate

```
aion-forge/
├── aion-analyze/     # 谱分析引擎
├── aion-refine/      # 迭代精炼闭环
├── aion-eval/        # 质量评估
└── aion-optimize/    # 超参优化器
```

## 依赖关系

```
aion-cli ──→ aion-router ──→ aion-intel
                    │                │
                    ↓                ↓
              aion-analyze ←──── aion-optimize
                    │                │
                    ↓                ↓
              aion-eval ←──────── aion-refine
```

## 接口设计原则

- 所有公开 API 必须有 `///` 文档注释
- 使用 `tracing` 代替 `println!`
- 数据结构加 `#[serde(default)]` 保证向后兼容
- 核心 trait 定义在 `aion-types` 中

## 实现顺序

1. **Phase 1**: `aion-types` 扩展（新 trait + 数据结构）
2. **Phase 2**: `aion-analyze`（谱分析引擎）
3. **Phase 3**: `aion-eval`（质量评估流水线）
4. **Phase 4**: `aion-refine`（迭代精炼闭环）
5. **Phase 5**: `aion-optimize`（超参优化器）
6. **Phase 6**: `aion-router` 集成（路由到新能力）
7. **Phase 7**: 测试 + 文档
