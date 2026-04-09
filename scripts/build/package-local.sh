#!/usr/bin/env bash
# 本地打包：将当前编译结果 + 配置文件打成一个 zip 技能包
# 用法: bash scripts/build/package-local.sh [--release]
# 输出: dist/aion-forge-v0.4.0-windows-x86_64.zip（当前平台）
set -euo pipefail

VERSION="v0.4.0"
PROFILE="debug"

if [[ "${1:-}" == "--release" ]]; then
  PROFILE="release"
fi

# 检测当前平台
case "$(uname -s 2>/dev/null || echo Windows)" in
  Linux*)  PLATFORM="linux-x86_64"; EXE="" ;;
  Darwin*)
    if [[ "$(uname -m)" == "arm64" ]]; then
      PLATFORM="macos-aarch64"
    else
      PLATFORM="macos-x86_64"
    fi
    EXE=""
    ;;
  *)       PLATFORM="windows-x86_64"; EXE=".exe" ;;
esac

PKG_NAME="aion-forge-${VERSION}-${PLATFORM}"
PKG_DIR="/tmp/${PKG_NAME}"
OUTPUT_DIR="dist"

echo "=== Packaging ${PKG_NAME} (${PROFILE}) ==="

rm -rf "$PKG_DIR"
mkdir -p "$PKG_DIR/adapters/claude-mcp"
mkdir -p "$PKG_DIR/adapters/openai"
mkdir -p "$PKG_DIR/adapters/http"
mkdir -p "$PKG_DIR/adapters/aionui"
mkdir -p "$OUTPUT_DIR"

# 复制二进制
CLI_BIN="target/${PROFILE}/aion-cli${EXE}"
SERVER_BIN="target/${PROFILE}/aion-server${EXE}"

if [[ ! -f "$CLI_BIN" ]]; then
  echo "Binary not found: $CLI_BIN"
  echo "Run: cargo build -p aion-cli -p aion-server [--release]"
  exit 1
fi

cp "$CLI_BIN" "$PKG_DIR/aion-cli${EXE}"
[[ -f "$SERVER_BIN" ]] && cp "$SERVER_BIN" "$PKG_DIR/aion-server${EXE}"

# 复制配置和文档
for f in safety-manifest.json skill.json manifest.json README.md CHANGELOG.md \
         SKILLS_GUIDE.md .env.example install.sh install.ps1; do
  [[ -f "$f" ]] && cp "$f" "$PKG_DIR/"
done

# 复制适配器
[[ -f "adapters/claude-mcp/claude_desktop_config.json" ]] && \
  cp "adapters/claude-mcp/claude_desktop_config.json" "$PKG_DIR/adapters/claude-mcp/"
[[ -f "adapters/README.md" ]] && cp "adapters/README.md" "$PKG_DIR/adapters/"

# 复制动态生成的适配器（如果存在）
for f in adapters/openai/functions.json adapters/http/openapi.json adapters/aionui/skill.json; do
  [[ -f "$f" ]] && cp "$f" "$PKG_DIR/$f"
done

# 打包
(cd /tmp && 7z a -tzip "${PKG_NAME}.zip" "${PKG_NAME}/" -r 2>/dev/null || zip -r "${PKG_NAME}.zip" "${PKG_NAME}/" 2>/dev/null || tar czf "${PKG_NAME}.tar.gz" "${PKG_NAME}/")

# 移到输出目录
for ext in zip tar.gz; do
  [[ -f "/tmp/${PKG_NAME}.${ext}" ]] && mv "/tmp/${PKG_NAME}.${ext}" "$OUTPUT_DIR/" && break
done

rm -rf "$PKG_DIR"

echo ""
echo "=== Package created ==="
ls -lh "$OUTPUT_DIR/"
echo ""
echo "Contents:"
if [[ -f "$OUTPUT_DIR/${PKG_NAME}.zip" ]]; then
  7z l "$OUTPUT_DIR/${PKG_NAME}.zip" 2>/dev/null || unzip -l "$OUTPUT_DIR/${PKG_NAME}.zip" 2>/dev/null || echo "(use 7z/unzip to list contents)"
fi
