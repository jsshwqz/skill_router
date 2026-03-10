# Skill Router v2.0.0 - opencode 快速安装脚本
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Skill Router v2.0.0 - opencode 安装" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

# 配置路径
$installDir = "D:\test\aionui\skill_router\v2.0.0"
$zipFile = "D:\test\aionui\skill_router\skill-router-v2.0.0.zip"

# 检查压缩包是否存在
if (-not (Test-Path $zipFile)) {
    Write-Host "错误: 找不到压缩包 $zipFile" -ForegroundColor Red
    exit 1
}

# 解压
Write-Host "解压文件..." -ForegroundColor Yellow
Expand-Archive -Path $zipFile -DestinationPath $installDir -Force

# 编译
Write-Host "编译项目..." -ForegroundColor Yellow
cd $installDir
cargo build --release

if ($LASTEXITCODE -ne 0) {
    Write-Host "编译失败" -ForegroundColor Red
    exit 1
}

# 创建快捷启动脚本
Add-Content -Path "$installDir\run.bat" -Value "@echo off"
Add-Content -Path "$installDir\run.bat" -Value "cd /d D:\test\aionui\skill_router\v2.0.0"
Add-Content -Path "$installDir\run.bat" -Value "cargo run --release -- %*"

# 输出结果
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "安装完成!" -ForegroundColor Green
Write-Host "安装路径: $installDir" -ForegroundColor White
Write-Host "使用方法:" -ForegroundColor Yellow
Write-Host "1. cd $installDir ; cargo run --release -- 'your task'" -ForegroundColor White
Write-Host "2. run.bat 'your task'" -ForegroundColor White
Write-Host "========================================" -ForegroundColor Cyan
