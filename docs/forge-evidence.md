# Forge 工作证据与验收命令

## 证据文件

- 执行事件日志（自动采集）  
  `C:\Users\Administrator\.aion\learning\execution_events.jsonl`
- 执行流水日志（路由执行落盘）  
  `.skill-router/executions.log`

## 快速验收命令

```powershell
# 1) 查看事件总数 + 最近 10 条
$p="$env:USERPROFILE\.aion\learning\execution_events.jsonl"
(Get-Content $p | Measure-Object -Line).Lines
Get-Content $p -Tail 10

# 2) 触发 Forge 执行
cargo run -q -p aion-cli -- --json "health check"
cargo run -q -p aion-cli -- --json "evolution report"
cargo run -q -p aion-cli -- --json "skill report"

# 3) 再看事件数是否增长
(Get-Content $p | Measure-Object -Line).Lines

# 4) 查看执行流水
Get-Content .skill-router/executions.log -Tail 10
```

## 判定标准

- 事件行数可增长（说明调用被自动采集）。
- `evolution report` 返回 `summary/sources/recommendations`（说明自动复盘可用）。
- `executions.log` 出现对应 capability 的 `status` 记录（说明执行链路可追踪）。

## 近期相关提交

- `cd76be3`：执行遥测 + 演化诊断能力
- `a22753f`：CLI/Server 初始化 learner
- `82de4f3`：wordcount 输入兼容 + 建议逻辑收敛
- `b297a79`：未修复失败追踪 + 最近窗口诊断
- `b7725d5`：自治策略接入路由前决策

