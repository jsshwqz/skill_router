#!/usr/bin/env python3
"""测试 Rust 版本 YAML 解析技能"""

import subprocess
import json

# 测试数据
test_yaml = """
name: "测试 YAML"
version: "1.0.0"
features:
  - logging
  - networking
  - security
config:
  debug: true
  max_connections: 100
"""

test_data = {
    "yaml": test_yaml.strip(),
    "validate": True
}

json_input = json.dumps(test_data, ensure_ascii=False)

cmd = [
    r"skills\yaml_parser\target\release\yaml_parser.exe",
    json_input
]

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
