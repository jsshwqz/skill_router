#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""错误日志技能测试脚本"""

import json
import subprocess
import sys
from pathlib import Path

PROJECT_DIR = Path(__file__).parent.resolve()
SKILL_DIR = PROJECT_DIR / "skills" / "error_logger"
MAIN_SCRIPT = SKILL_DIR / "main.py"


def test_skill_json():
    """验证技能配置"""
    print("\n" + "=" * 60)
    print("测试 1: 技能配置验证")
    print("=" * 60)
    
    skill_json = SKILL_DIR / "skill.json"
    
    if not skill_json.exists():
        print("[FAIL] skill.json 不存在")
        return False
    
    try:
        with open(skill_json, 'r', encoding='utf-8') as f:
            config = json.load(f)
        
        required_keys = ['name', 'version', 'description', 'capabilities', 'permissions', 'entrypoint']
        for key in required_keys:
            if key not in config:
                print(f"[FAIL] 缺少必需字段: {key}")
                return False
        
        print("[OK] skill.json 配置完整")
        print(f"   技能名称: {config['name']}")
        print(f"   版本: {config['version']}")
        print(f"   能力: {', '.join(config['capabilities'])}")
        return True
    except json.JSONDecodeError:
        print("[FAIL] skill.json 格式错误")
        return False


def test_basic_error():
    """测试基本错误记录"""
    print("\n" + "=" * 60)
    print("测试 2: 基本错误记录")
    print("=" * 60)
    
    error_data = {
        "title": "PowerShell 命令连接符错误",
        "type": "语法错误",
        "affected": "所有 Windows 用户",
        "severity": "中等",
        "symptom": "用户执行命令后未看到预期输出",
        "root_cause": "在 PowerShell 中使用了 && 连接符（Bash 语法）",
        "solution": "改用分号 ; 分隔命令",
        "verification": "cargo build --release 验证通过",
        "checklist": [
            "确认在项目根目录",
            "确认 Rust 已安装"
        ],
        "notify": False
    }
    
    result = subprocess.run(
        [sys.executable, str(MAIN_SCRIPT), json.dumps(error_data, ensure_ascii=False)],
        capture_output=True,
        text=True
    )
    
    print(result.stdout)
    if result.stderr:
        print(result.stderr)
    
    return result.returncode == 0


def test_existing_error():
    """测试重复记录"""
    print("\n" + "=" * 60)
    print("测试 3: 重复记录测试")
    print("=" * 60)
    
    error_data = {
        "title": "重复错误记录测试",
        "type": "重复测试",
        "symptom": "测试重复记录功能",
        "root_cause": "重复调用错误记录技能",
        "solution": "应该追加到现有日志",
        "verification": "ERROR_LOG.md 中应有两条记录",
        "notify": False
    }
    
    result = subprocess.run(
        [sys.executable, str(MAIN_SCRIPT), json.dumps(error_data, ensure_ascii=False)],
        capture_output=True,
        text=True
    )
    
    print(result.stdout)
    if result.stderr:
        print(result.stderr)
    
    return result.returncode == 0


def test_verify_documentation():
    """验证文档更新"""
    print("\n" + "=" * 60)
    print("测试 4: 文档更新验证")
    print("=" * 60)
    
    error_log = PROJECT_DIR / "ERROR_LOG.md"
    
    if not error_log.exists():
        print("[FAIL] ERROR_LOG.md 不存在")
        return False
    
    content = error_log.read_text(encoding='utf-8')
    
    if "## 错误" not in content:
        print("[FAIL] ERROR_LOG.md 中没有错误记录")
        return False
    
    if "**最后更新**" not in content and "最后更新" not in content:
        print("[FAIL] ERROR_LOG.md 中没有最后更新时间")
        return False
    
    print("[OK] ERROR_LOG.md 更新正常")
    print(f"   文件大小: {error_log.stat().st_size} 字节")
    return True


def test_rust_build():
    """测试 Rust 项目构建"""
    print("\n" + "=" * 60)
    print("测试 5: Rust 构建验证")
    print("=" * 60)
    
    result = subprocess.run(
        ["cargo", "build", "--release"],
        cwd=str(PROJECT_DIR),
        capture_output=True,
        text=True
    )
    
    if result.returncode == 0:
        print("[OK] cargo build --release 成功")
        return True
    else:
        print(f"[FAIL] cargo build --release 失败")
        print(result.stderr)
        return False


def main():
    """运行所有测试"""
    print("\n" + "=" * 60)
    print("错误日志技能测试套件")
    print("=" * 60)
    
    tests = [
        ("技能配置验证", test_skill_json),
        ("基本错误记录", test_basic_error),
        ("重复记录测试", test_existing_error),
        ("文档更新验证", test_verify_documentation),
        ("Rust 构建验证", test_rust_build),
    ]
    
    results = []
    for name, test_func in tests:
        try:
            passed = test_func()
            results.append((name, passed))
        except Exception as e:
            print(f"[FAIL] 测试异常: {e}")
            results.append((name, False))
    
    print("\n" + "=" * 60)
    print("测试总结")
    print("=" * 60)
    
    passed = sum(1 for _, p in results if p)
    total = len(results)
    
    for name, status in results:
        symbol = "[OK]" if status else "[FAIL]"
        print(f"{symbol} {name}")
    
    print(f"\n总计: {passed}/{total} 通过")
    print("=" * 60)
    
    return passed == total


if __name__ == "__main__":
    success = main()
    sys.exit(0 if success else 1)
