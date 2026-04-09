#!/usr/bin/env bash
# CI 脚本：将二进制 SHA256 校验和写入 safety-manifest.json
# 用法: bash scripts/update-checksums.sh <artifacts-dir> <manifest-file>
set -euo pipefail

ARTIFACTS_DIR="${1:?Usage: update-checksums.sh <artifacts-dir> <manifest-file>}"
MANIFEST="${2:?Usage: update-checksums.sh <artifacts-dir> <manifest-file>}"

if [[ ! -f "$MANIFEST" ]]; then
  echo "ERROR: $MANIFEST not found"
  exit 1
fi

echo "Updating checksums in $MANIFEST..."

# 定义二进制名称到 JSON 路径的映射
declare -A BINARY_MAP=(
  ["aion-cli-linux-x86_64"]="aion-cli.platforms.x86_64-unknown-linux-musl"
  ["aion-cli-windows-x86_64.exe"]="aion-cli.platforms.x86_64-pc-windows-msvc"
  ["aion-cli-macos-x86_64"]="aion-cli.platforms.x86_64-apple-darwin"
  ["aion-cli-macos-aarch64"]="aion-cli.platforms.aarch64-apple-darwin"
  ["aion-server-linux-x86_64"]="aion-server.platforms.x86_64-unknown-linux-musl"
  ["aion-server-windows-x86_64.exe"]="aion-server.platforms.x86_64-pc-windows-msvc"
  ["aion-server-macos-x86_64"]="aion-server.platforms.x86_64-apple-darwin"
  ["aion-server-macos-aarch64"]="aion-server.platforms.aarch64-apple-darwin"
)

# 为每个二进制计算 SHA256 并用 python3 更新 JSON
for artifact_name in "${!BINARY_MAP[@]}"; do
  json_path="${BINARY_MAP[$artifact_name]}"
  binary_name="${json_path%%.*}"       # e.g. "aion-cli"
  platform_path="${json_path#*.}"      # e.g. "platforms.x86_64-unknown-linux-musl"
  platform="${platform_path#*.}"       # e.g. "x86_64-unknown-linux-musl"

  # 查找文件（可能在子目录中）
  file_path=$(find "$ARTIFACTS_DIR" -name "$artifact_name" -type f 2>/dev/null | head -1)

  if [[ -z "$file_path" ]]; then
    echo "  SKIP: $artifact_name not found in $ARTIFACTS_DIR"
    continue
  fi

  sha256=$(sha256sum "$file_path" | cut -d' ' -f1)
  echo "  $artifact_name → $sha256"

  # 用 python3 更新 JSON（jq 可能不存在于所有 runner）
  python3 -c "
import json
with open('$MANIFEST', 'r') as f:
    m = json.load(f)
m['binaries']['$binary_name']['platforms']['$platform']['sha256'] = '$sha256'
with open('$MANIFEST', 'w') as f:
    json.dump(m, f, indent=2, ensure_ascii=False)
"
done

echo "Done. Updated $MANIFEST"
