# Aion Forge — 一键安装脚本 (Windows PowerShell)
# 用法: irm https://raw.githubusercontent.com/aioncore/aion-forge/main/install.ps1 | iex
# 或者: .\install.ps1 [-Version vX.Y.Z] [-Yes]
param(
    [string]$Version = "",
    [switch]$Yes
)

$ErrorActionPreference = "Stop"

# ─── 配置 ───
$Repo = "aioncore/aion-forge"
$InstallDir = "$env:USERPROFILE\.aion\bin"
$ConfigDir = "$env:USERPROFILE\.aion"

# ─── 辅助函数 ───
function Write-Info  { Write-Host "[INFO]  $args" -ForegroundColor Blue }
function Write-Ok    { Write-Host "[OK]    $args" -ForegroundColor Green }
function Write-Warn  { Write-Host "[WARN]  $args" -ForegroundColor Yellow }
function Write-Err   { Write-Host "[ERROR] $args" -ForegroundColor Red; exit 1 }

# ─── 获取版本 ───
function Get-LatestVersion {
    if ($Version) {
        Write-Info "Using specified version: $Version"
        return $Version
    }

    Write-Info "Fetching latest release version..."
    try {
        $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -Headers @{ "User-Agent" = "aion-installer" }
        $Version = $release.tag_name
        Write-Info "Latest version: $Version"
        return $Version
    }
    catch {
        Write-Err "Failed to fetch latest version. Please specify with -Version vX.Y.Z"
    }
}

# ─── 安全审查 ───
function Show-SafetyReview {
    param([string]$Ver)

    $manifestUrl = "https://github.com/$Repo/releases/download/$Ver/safety-manifest.json"
    $manifestFile = "$env:TEMP\aion-safety-manifest.json"

    Write-Info "Downloading safety manifest..."
    try {
        Invoke-WebRequest -Uri $manifestUrl -OutFile $manifestFile -UseBasicParsing
    }
    catch {
        Write-Warn "Could not download safety-manifest.json (may not exist for this version)"
        Write-Warn "Proceeding without safety verification"
        return
    }

    $manifest = Get-Content $manifestFile | ConvertFrom-Json

    Write-Host ""
    Write-Host "  ╔══════════════════════════════════════════════════════╗" -ForegroundColor Cyan
    Write-Host "  ║         Aion Forge — Safety Review / 安全审查        ║" -ForegroundColor Cyan
    Write-Host "  ╚══════════════════════════════════════════════════════╝" -ForegroundColor Cyan
    Write-Host ""

    $rating = $manifest.security.safety_rating
    Write-Host "  Safety Rating / 安全评分:  " -NoNewline
    Write-Host "$rating/5" -ForegroundColor Green
    Write-Host ""

    Write-Host "  Permissions Required / 所需权限:" -ForegroundColor Yellow
    Write-Host "    ✓ Network Access / 网络访问" -ForegroundColor Green
    Write-Host "      → AI model calls + web search API"
    Write-Host "    ✓ File Read / 文件读取" -ForegroundColor Green
    Write-Host "      → Workspace files for parsing"
    Write-Host "    ✓ File Write / 文件写入" -ForegroundColor Green
    Write-Host "      → Memory store + audit logs (.skill-router/ only)"
    Write-Host "    ✗ Process Execution / 进程执行" -ForegroundColor Red
    Write-Host "      → Disabled by default / 默认禁用"
    Write-Host ""

    Write-Host "  Runtime Security / 运行时安全:" -ForegroundColor Yellow
    Write-Host "    ✓ Pre-execution review / 预执行审查" -ForegroundColor Green
    Write-Host "    ✓ Post-execution review / 后执行审查" -ForegroundColor Green
    Write-Host "    ✓ SSRF protection / SSRF 防护" -ForegroundColor Green
    Write-Host "    ✓ Audit logging / 审计日志" -ForegroundColor Green
    Write-Host "    ✓ Fail-closed policy / 默认拒绝策略" -ForegroundColor Green
    Write-Host ""

    if (-not $Yes) {
        $confirm = Read-Host "  Accept and continue? / 接受并继续？ [y/N]"
        if ($confirm -notmatch "^[yY]") {
            Write-Host "  Installation cancelled."
            exit 0
        }
    }
    else {
        Write-Ok "Auto-confirmed (-Yes flag)"
    }
}

# ─── SHA256 校验 ───
function Test-Checksum {
    param([string]$File, [string]$Target)

    $manifestFile = "$env:TEMP\aion-safety-manifest.json"
    if (-not (Test-Path $manifestFile)) {
        Write-Warn "No safety manifest, skipping checksum"
        return
    }

    $manifest = Get-Content $manifestFile | ConvertFrom-Json
    $expected = $null

    foreach ($binary in @("aion-cli", "aion-server")) {
        $platforms = $manifest.binaries.$binary.platforms
        if ($platforms.$Target) {
            $sha = $platforms.$Target.sha256
            if ($sha -and $sha -ne "TO_BE_FILLED_BY_CI") {
                $actual = (Get-FileHash -Path $File -Algorithm SHA256).Hash.ToLower()
                if ($actual -ne $sha.ToLower()) {
                    Remove-Item $File -Force
                    Write-Err @"
SHA256 CHECKSUM MISMATCH! / 校验和不匹配！
  Expected: $sha
  Got:      $actual
  The downloaded binary may be corrupted or tampered with.
  下载的文件可能已损坏或被篡改。
  Installation aborted. / 安装已中止。
"@
                }
                Write-Ok "SHA256 verified for $File"
                return
            }
        }
    }

    Write-Warn "No checksum available, skipping verification"
}

# ─── 主流程 ───
function Main {
    Write-Host ""
    Write-Host "  ╔═══════════════════════════════════════╗" -ForegroundColor Cyan
    Write-Host "  ║   Aion Forge Installer / 安装程序     ║" -ForegroundColor Cyan
    Write-Host "  ╚═══════════════════════════════════════╝" -ForegroundColor Cyan
    Write-Host ""

    # 平台检测
    $arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
    Write-Info "Detected platform: Windows $arch"

    $cliArtifact = "aion-cli-windows-x86_64.exe"
    $serverArtifact = "aion-server-windows-x86_64.exe"
    $target = "x86_64-pc-windows-msvc"

    # 获取版本
    $ver = Get-LatestVersion

    # 安全审查
    Show-SafetyReview -Ver $ver

    # 创建目录
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }
    if (-not (Test-Path $ConfigDir)) {
        New-Item -ItemType Directory -Path $ConfigDir -Force | Out-Null
    }

    # 下载二进制
    $baseUrl = "https://github.com/$Repo/releases/download/$ver"

    Write-Info "Downloading aion-cli..."
    Invoke-WebRequest -Uri "$baseUrl/$cliArtifact" -OutFile "$InstallDir\aion-cli.exe" -UseBasicParsing

    Write-Info "Downloading aion-server..."
    Invoke-WebRequest -Uri "$baseUrl/$serverArtifact" -OutFile "$InstallDir\aion-server.exe" -UseBasicParsing

    # SHA256 校验
    Test-Checksum -File "$InstallDir\aion-cli.exe" -Target $target
    Test-Checksum -File "$InstallDir\aion-server.exe" -Target $target

    # 配置 PATH
    $currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($currentPath -notlike "*$InstallDir*") {
        [Environment]::SetEnvironmentVariable("Path", "$InstallDir;$currentPath", "User")
        $env:Path = "$InstallDir;$env:Path"
        Write-Info "Added $InstallDir to user PATH"
    }

    # 创建默认 .env
    $envFile = "$ConfigDir\.env"
    if (-not (Test-Path $envFile)) {
        @"
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
"@ | Out-File -FilePath $envFile -Encoding utf8
        Write-Ok "Created config file: $envFile"
    }

    # 验证安装
    Write-Host ""
    Write-Info "Verifying installation... / 验证安装..."
    try {
        & "$InstallDir\aion-cli.exe" --help | Out-Null
        Write-Ok "aion-cli is working / aion-cli 正常工作"
    }
    catch {
        Write-Warn "aion-cli verification failed"
    }

    # 完成
    Write-Host ""
    Write-Host "  ╔═══════════════════════════════════════════════════════╗" -ForegroundColor Green
    Write-Host "  ║        Installation Complete! / 安装完成！            ║" -ForegroundColor Green
    Write-Host "  ╚═══════════════════════════════════════════════════════╝" -ForegroundColor Green
    Write-Host ""
    Write-Host "  Version / 版本:    " -NoNewline; Write-Host "$ver" -ForegroundColor Cyan
    Write-Host "  Install path / 路径: " -NoNewline; Write-Host "$InstallDir" -ForegroundColor Cyan
    Write-Host "  Config / 配置:     " -NoNewline; Write-Host "$envFile" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "  Quick Start / 快速开始:" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "    # Restart terminal, then / 重启终端后:"
    Write-Host "    aion-cli echo `"hello world`""
    Write-Host ""
    Write-Host "    # Start HTTP API / 启动 HTTP API:"
    Write-Host "    aion-server"
    Write-Host ""
    Write-Host "    # Edit config / 编辑配置:"
    Write-Host "    notepad $envFile"
    Write-Host ""
    Write-Host "  AI Platform Integration / AI 平台集成:" -ForegroundColor Yellow
    Write-Host "    aionui  -> Drop skill.json into skills directory"
    Write-Host "    Claude  -> aion-cli mcp-server (MCP stdio)"
    Write-Host "    ChatGPT -> Import openapi.yaml as Custom GPT Action"
    Write-Host "    HTTP    -> curl http://localhost:3000/v1/route"
    Write-Host ""
}

Main
