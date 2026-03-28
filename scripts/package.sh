#!/usr/bin/env bash
# 打包脚本：生成各平台的完整技能包 (.zip)
# 用法: bash scripts/package.sh <artifacts-dir> <version>
# 输出: aion-forge-<platform>.zip（每个平台一个，包含二进制+配置+文档）
set -euo pipefail

ARTIFACTS_DIR="${1:?Usage: package.sh <artifacts-dir> <version>}"
VERSION="${2:?Usage: package.sh <artifacts-dir> <version>}"
OUTPUT_DIR="dist"

mkdir -p "$OUTPUT_DIR"

# 平台映射
declare -A PLATFORMS=(
  ["linux-x86_64"]="x86_64-unknown-linux-musl"
  ["windows-x86_64"]="x86_64-pc-windows-msvc"
  ["macos-x86_64"]="x86_64-apple-darwin"
  ["macos-aarch64"]="aarch64-apple-darwin"
)

# 共享文件列表（每个包都包含）
SHARED_FILES=(
  "safety-manifest.json"
  "skill.json"
  "manifest.json"
  "README.md"
  "CHANGELOG.md"
  "SKILLS_GUIDE.md"
  ".env.example"
  "install.sh"
  "install.ps1"
  "adapters/README.md"
  "adapters/claude-mcp/claude_desktop_config.json"
)

for platform in "${!PLATFORMS[@]}"; do
  target="${PLATFORMS[$platform]}"
  pkg_name="aion-forge-${VERSION}-${platform}"
  pkg_dir="/tmp/${pkg_name}"

  echo "=== Packaging ${pkg_name} ==="

  rm -rf "$pkg_dir"
  mkdir -p "$pkg_dir/adapters/claude-mcp"
  mkdir -p "$pkg_dir/adapters/openai"
  mkdir -p "$pkg_dir/adapters/http"
  mkdir -p "$pkg_dir/adapters/aionui"

  # 复制共享文件
  for f in "${SHARED_FILES[@]}"; do
    if [[ -f "$f" ]]; then
      cp "$f" "$pkg_dir/$f"
    fi
  done

  # 复制二进制（从 artifacts 目录查找）
  if [[ "$platform" == windows* ]]; then
    cli_file=$(find "$ARTIFACTS_DIR" -name "aion-cli-${platform}.exe" -type f 2>/dev/null | head -1)
    server_file=$(find "$ARTIFACTS_DIR" -name "aion-server-${platform}.exe" -type f 2>/dev/null | head -1)
    [[ -n "$cli_file" ]] && cp "$cli_file" "$pkg_dir/aion-cli.exe"
    [[ -n "$server_file" ]] && cp "$server_file" "$pkg_dir/aion-server.exe"
  else
    cli_file=$(find "$ARTIFACTS_DIR" -name "aion-cli-${platform}" -type f 2>/dev/null | head -1)
    server_file=$(find "$ARTIFACTS_DIR" -name "aion-server-${platform}" -type f 2>/dev/null | head -1)
    [[ -n "$cli_file" ]] && { cp "$cli_file" "$pkg_dir/aion-cli"; chmod +x "$pkg_dir/aion-cli"; }
    [[ -n "$server_file" ]] && { cp "$server_file" "$pkg_dir/aion-server"; chmod +x "$pkg_dir/aion-server"; }
  fi

  # 如果有动态生成的适配器文件也复制
  for f in adapters/openai/functions.json adapters/http/openapi.json adapters/aionui/skill.json; do
    [[ -f "$f" ]] && cp "$f" "$pkg_dir/$f"
  done

  # 打包
  (cd /tmp && zip -r "${pkg_name}.zip" "${pkg_name}/")
  mv "/tmp/${pkg_name}.zip" "$OUTPUT_DIR/"
  rm -rf "$pkg_dir"

  echo "  -> ${OUTPUT_DIR}/${pkg_name}.zip"
done

echo ""
echo "=== All packages created in ${OUTPUT_DIR}/ ==="
ls -lh "$OUTPUT_DIR/"
