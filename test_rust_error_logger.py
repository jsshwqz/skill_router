#!/usr/bin/env python3
"""测试 Rust 版本错误日志技能"""

import subprocess
import json

# 测试数据
test_data = {
    "title": "Rust版本技能测试",
    "type": "功能测试",
    "affected": "所有用户",
    "symptom": "测试Rust版本是否正常",
    "root_cause": "测试",
    "solution": "验证通过",
    "verification": "测试成功",
    "checklist": ["测试项1", "测试项2"],
    "notify": True
}

# 转换为 JSON 字符串
json_input = json.dumps(test_data, ensure_ascii=False)

# 构建命令
cmd = [
    r"skills\error_logger\target\release\error_logger.exe",
    json_input
]

# 运行测试
try:
    result = subprocess.run(
        cmd,
        capture_output=True,
        text=True,
        encoding='utf-8',
        errors='replace'
    )
    
    print("=== 测试结果 ===")
    print("stdout:")
    print(result.stdout)
    if result.stderr:
        print("\nstderr:")
        print(result.stderr)
    print(f"\nExit Code: {result.returncode}")
    
except Exception as e:
    print(f"错误: {e}")
