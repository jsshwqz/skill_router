#!/usr/bin/env bash
# Aion Forge — 一键安装脚本 (Mac/Linux)
# 用法: curl -fsSL https://raw.githubusercontent.com/aioncore/aion-forge/main/install.sh | bash
# 或者: bash install.sh [--version vX.Y.Z] [--yes]
set -euo pipefail

# ─── 配置 ───
REPO="aioncore/aion-forge"
INSTALL_DIR="${HOME}/.aion/bin"
CONFIG_DIR="${HOME}/.aion"
VERSION=""
AUTO_YES=false

# ─── 颜色输出 ───
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { echo -e "${BLUE}[INFO]${NC}  $1"; }
ok()    { echo -e "${GREEN}[OK]${NC}    $1"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# ─── 参数解析 ───
while [[ $# -gt 0 ]]; do
  case $1 in
    --version) VERSION="$2"; shift 2 ;;
    --yes|-y)  AUTO_YES=true; shift ;;
    --help|-h)
      echo "Aion Forge Installer"
      echo "Usage: install.sh [--version vX.Y.Z] [--yes]"
      echo "  --version  Install specific version (default: latest)"
      echo "  --yes      Skip safety confirmation prompt"
      exit 0 ;;
    *) error "Unknown option: $1" ;;
  esac
done

# ─── 平台检测 ───
detect_platform() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux*)  OS="linux" ;;
    Darwin*) OS="macos" ;;
    *)       error "Unsupported OS: $os (only Linux and macOS are supported)" ;;
  esac

  case "$arch" in
    x86_64|amd64)   ARCH="x86_64" ;;
    arm64|aarch64)   ARCH="aarch64" ;;
    *)               error "Unsupported architecture: $arch" ;;
  esac

  # 构建 artifact 名称
  if [[ "$OS" == "linux" ]]; then
    CLI_ARTIFACT="aion-cli-linux-x86_64"
    SERVER_ARTIFACT="aion-server-linux-x86_64"
    TARGET="x86_64-unknown-linux-musl"
  elif [[ "$OS" == "macos" && "$ARCH" == "aarch64" ]]; then
    CLI_ARTIFACT="aion-cli-macos-aarch64"
    SERVER_ARTIFACT="aion-server-macos-aarch64"
    TARGET="aarch64-apple-darwin"
  else
    CLI_ARTIFACT="aion-cli-macos-x86_64"
    SERVER_ARTIFACT="aion-server-macos-x86_64"
    TARGET="x86_64-apple-darwin"
  fi

  info "Detected platform: ${OS} ${ARCH} (${TARGET})"
}

# ─── 获取最新版本 ───
get_version() {
  if [[ -n "$VERSION" ]]; then
    info "Using specified version: ${VERSION}"
    return
  fi

  info "Fetching latest release version..."
  VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | head -1 | sed -E 's/.*"tag_name":\s*"([^"]+)".*/\1/')

  if [[ -z "$VERSION" ]]; then
    error "Failed to fetch latest version. Please specify with --version vX.Y.Z"
  fi

  info "Latest version: ${VERSION}"
}

# ─── 下载文件 ───
download() {
  local url="$1" dest="$2"
  if command -v curl &>/dev/null; then
    curl -fsSL "$url" -o "$dest"
  elif command -v wget &>/dev/null; then
    wget -q "$url" -O "$dest"
  else
    error "Neither curl nor wget found. Please install one of them."
  fi
}

# ─── 安全审查 ───
safety_review() {
  local manifest_url="https://github.com/${REPO}/releases/download/${VERSION}/safety-manifest.json"
  local manifest_file="/tmp/aion-safety-manifest.json"

  info "Downloading safety manifest..."
  if ! download "$manifest_url" "$manifest_file" 2>/dev/null; then
    warn "Could not download safety-manifest.json (may not exist for this version)"
    warn "Proceeding without safety verification"
    return
  fi

  echo ""
  echo -e "${CYAN}╔══════════════════════════════════════════════════════╗${NC}"
  echo -e "${CYAN}║         Aion Forge — Safety Review / 安全审查        ║${NC}"
  echo -e "${CYAN}╚══════════════════════════════════════════════════════╝${NC}"
  echo ""

  # 解析权限信息
  local safety_rating
  safety_rating=$(python3 -c "import json; m=json.load(open('$manifest_file')); print(m['security']['safety_rating'])" 2>/dev/null || echo "N/A")

  echo -e "  Safety Rating / 安全评分:  ${GREEN}${safety_rating}/5${NC}"
  echo ""
  echo -e "  ${YELLOW}Permissions Required / 所需权限:${NC}"
  echo -e "    ${GREEN}✓${NC} Network Access / 网络访问"
  echo -e "      → AI model calls + web search API"
  echo -e "      → AI 模型调用 + 网页搜索 API"
  echo -e "    ${GREEN}✓${NC} File Read / 文件读取"
  echo -e "      → Workspace files for parsing"
  echo -e "      → 读取工作区文件用于解析"
  echo -e "    ${GREEN}✓${NC} File Write / 文件写入"
  echo -e "      → Memory store + audit logs (in .skill-router/ only)"
  echo -e "      → 记忆存储 + 审计日志（仅 .skill-router/ 目录）"
  echo -e "    ${RED}✗${NC} Process Execution / 进程执行"
  echo -e "      → Disabled by default"
  echo -e "      → 默认禁用"
  echo ""
  echo -e "  ${YELLOW}Runtime Security / 运行时安全:${NC}"
  echo -e "    ${GREEN}✓${NC} Pre-execution review (heuristic + AI) / 预执行审查"
  echo -e "    ${GREEN}✓${NC} Post-execution review (secret leak detection) / 后执行审查"
  echo -e "    ${GREEN}✓${NC} SSRF protection (private network blocking) / SSRF 防护"
  echo -e "    ${GREEN}✓${NC} Audit logging / 审计日志"
  echo -e "    ${GREEN}✓${NC} Fail-closed policy (default) / 默认拒绝策略"
  echo ""

  if [[ "$AUTO_YES" == true ]]; then
    ok "Auto-confirmed (--yes flag)"
    return
  fi

  read -rp "$(echo -e "${CYAN}Accept and continue installation? / 接受并继续安装？ [y/N]: ${NC}")" confirm
  case "$confirm" in
    [yY]|[yY][eE][sS]) ok "Accepted" ;;
    *) echo "Installation cancelled."; exit 0 ;;
  esac
}

# ─── SHA256 校验 ───
verify_checksum() {
  local file="$1" expected_target="$2"
  local manifest_file="/tmp/aion-safety-manifest.json"

  if [[ ! -f "$manifest_file" ]]; then
    warn "No safety manifest available, skipping checksum verification"
    return 0
  fi

  local expected
  expected=$(python3 -c "
import json, sys
m = json.load(open('$manifest_file'))
for binary in m.get('binaries', {}).values():
    platforms = binary.get('platforms', {})
    if '$expected_target' in platforms:
        sha = platforms['$expected_target'].get('sha256', 'TO_BE_FILLED_BY_CI')
        if sha != 'TO_BE_FILLED_BY_CI':
            print(sha)
            sys.exit(0)
print('')
" 2>/dev/null || echo "")

  if [[ -z "$expected" || "$expected" == "TO_BE_FILLED_BY_CI" ]]; then
    warn "No checksum available for ${expected_target}, skipping verification"
    return 0
  fi

  local actual
  if command -v sha256sum &>/dev/null; then
    actual=$(sha256sum "$file" | cut -d' ' -f1)
  elif command -v shasum &>/dev/null; then
    actual=$(shasum -a 256 "$file" | cut -d' ' -f1)
  else
    warn "No sha256sum/shasum tool found, skipping checksum verification"
    return 0
  fi

  if [[ "$actual" != "$expected" ]]; then
    error "SHA256 CHECKSUM MISMATCH! / 校验和不匹配！
  Expected: ${expected}
  Got:      ${actual}
  The downloaded binary may be corrupted or tampered with.
  下载的文件可能已损坏或被篡改。
  Installation aborted. / 安装已中止。"
  fi

  ok "SHA256 checksum verified / 校验和验证通过"
}

# ─── 主流程 ───
main() {
  echo ""
  echo -e "${CYAN}  ╔═══════════════════════════════════════╗${NC}"
  echo -e "${CYAN}  ║   Aion Forge Installer / 安装程序     ║${NC}"
  echo -e "${CYAN}  ╚═══════════════════════════════════════╝${NC}"
  echo ""

  detect_platform
  get_version

  # Step 1: 安全审查
  safety_review

  # Step 2: 创建目录
  mkdir -p "$INSTALL_DIR"
  mkdir -p "$CONFIG_DIR"

  # Step 3: 下载二进制
  local base_url="https://github.com/${REPO}/releases/download/${VERSION}"

  info "Downloading aion-cli..."
  download "${base_url}/${CLI_ARTIFACT}" "${INSTALL_DIR}/aion-cli"
  chmod +x "${INSTALL_DIR}/aion-cli"

  info "Downloading aion-server..."
  download "${base_url}/${SERVER_ARTIFACT}" "${INSTALL_DIR}/aion-server"
  chmod +x "${INSTALL_DIR}/aion-server"

  # Step 4: SHA256 校验
  verify_checksum "${INSTALL_DIR}/aion-cli" "$TARGET"
  verify_checksum "${INSTALL_DIR}/aion-server" "$TARGET"

  # Step 5: 配置 PATH
  if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
    local shell_rc=""
    if [[ -n "${ZSH_VERSION:-}" ]] || [[ "$SHELL" == *"zsh"* ]]; then
      shell_rc="$HOME/.zshrc"
    elif [[ -f "$HOME/.bashrc" ]]; then
      shell_rc="$HOME/.bashrc"
    elif [[ -f "$HOME/.bash_profile" ]]; then
      shell_rc="$HOME/.bash_profile"
    fi

    if [[ -n "$shell_rc" ]]; then
      echo "" >> "$shell_rc"
      echo "# Aion Forge" >> "$shell_rc"
      echo "export PATH=\"${INSTALL_DIR}:\$PATH\"" >> "$shell_rc"
      info "Added ${INSTALL_DIR} to PATH in ${shell_rc}"
    fi

    export PATH="${INSTALL_DIR}:$PATH"
  fi

  # Step 6: 创建默认 .env（如果不存在）
  if [[ ! -f "${CONFIG_DIR}/.env" ]]; then
    cat > "${CONFIG_DIR}/.env" << 'ENVEOF'
# Aion Forge Configuration / 配置文件
# Docs: https://github.com/aioncore/aion-forge

# AI Backend / AI 后端配置
AI_BASE_URL=http://localhost:11434/v1
AI_API_KEY=
AI_MODEL=qwen2.5:7b

# Web Search (optional) / 网页搜索（可选）
SERPAPI_KEY=

# Server / 服务器配置
AION_HOST=0.0.0.0
AION_PORT=3000

# Security / 安全策略
AI_SECURITY_FAIL_POLICY=closed

# Logging / 日志
RUST_LOG=info
ENVEOF
    ok "Created config file: ${CONFIG_DIR}/.env"
  fi

  # Step 7: 验证安装
  echo ""
  info "Verifying installation... / 验证安装..."
  if "${INSTALL_DIR}/aion-cli" --help &>/dev/null; then
    ok "aion-cli is working / aion-cli 正常工作"
  else
    warn "aion-cli verification failed — binary may not be compatible with this system"
  fi

  # Step 8: 完成
  echo ""
  echo -e "${GREEN}╔═══════════════════════════════════════════════════════╗${NC}"
  echo -e "${GREEN}║        Installation Complete! / 安装完成！            ║${NC}"
  echo -e "${GREEN}╚═══════════════════════════════════════════════════════╝${NC}"
  echo ""
  echo -e "  Version / 版本:    ${CYAN}${VERSION}${NC}"
  echo -e "  Install path / 路径: ${CYAN}${INSTALL_DIR}${NC}"
  echo -e "  Config / 配置:     ${CYAN}${CONFIG_DIR}/.env${NC}"
  echo ""
  echo -e "  ${YELLOW}Quick Start / 快速开始:${NC}"
  echo ""
  echo -e "    # Reload shell / 重新加载终端"
  echo -e "    source ~/.bashrc  # or ~/.zshrc"
  echo ""
  echo -e "    # Try it / 试一试"
  echo -e "    aion-cli echo \"hello world\""
  echo ""
  echo -e "    # Start HTTP API server / 启动 HTTP API"
  echo -e "    aion-server"
  echo ""
  echo -e "    # Edit config / 编辑配置"
  echo -e "    nano ${CONFIG_DIR}/.env"
  echo ""
  echo -e "  ${YELLOW}AI Platform Integration / AI 平台集成:${NC}"
  echo -e "    aionui  → Drop skill.json into skills directory"
  echo -e "    Claude  → aion-cli mcp-server (MCP stdio)"
  echo -e "    ChatGPT → Import openapi.yaml as Custom GPT Action"
  echo -e "    HTTP    → curl http://localhost:3000/v1/route"
  echo ""
}

main "$@"
