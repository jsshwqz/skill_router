import sys
import subprocess
import os
import json

def call_router(task):
    """递归调用路由器，完成原子任务"""
    router_path = os.path.join(os.getcwd(), "skill-router.exe")
    try:
        res = subprocess.run(
            [router_path, "--json", task],
            capture_output=True,
            text=True
        )
        return json.loads(res.stdout) if res.stdout else {"status": "error", "message": "Empty output"}
    except Exception as e:
        return {"status": "error", "message": str(e)}

def decompose_task(task):
    """
    智能拆解任务。
    在真实场景中，这里会调用 LLM API 获取分步计划。
    这里展示的是逻辑骨架。
    """
    print(f"[ORCHESTRATOR] 分析任务: {task}")
    
    # 示例拆解逻辑：
    if "汇总" in task and "文件" in task:
        return [
            f"1. 搜索是否有文件遍历能力的技能处理任务: {task}",
            f"2. 执行文件分析与统计"
        ]
    return [task] # 若太简单，直接作为单一任务执行

def main():
    if len(sys.argv) < 2:
        print("Usage: python main.py '<complex_task>'")
        sys.exit(1)
        
    complex_task = sys.argv[1]
    
    # 1. 分析与拆解
    steps = decompose_task(complex_task)
    print(f"[ORCHESTRATOR] 拆解为 {len(steps)} 个子任务。")
    
    context = {}
    
    # 2. 依次执行子任务
    for i, step in enumerate(steps):
        print(f"
[STEP {i+1}/{len(steps)}] 开始执行: {step}")
        
        # 3. 调度 Skill Router (它内部已经实现了: 本地优先 -> 网上查找 -> 现场合成)
        # 这就是您要求的“无感”核心
        result = call_router(step)
        
        if result.get("status") == "success":
            print(f"[STEP {i+1}] 成功: {result.get('skill', 'unknown_skill')}")
            context[f"step_{i+1}"] = result
        else:
            print(f"[STEP {i+1}] 失败: {result.get('message', '未知错误')}")
            # 这里可以增加“重试”或“修正策略”
            break

    print("
[ORCHESTRATOR] 任务流程执行完毕。")

if __name__ == "__main__":
    main()
