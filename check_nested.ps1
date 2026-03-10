# 检查每个版本的实际差异

$versions = @(
    @{Name="v0.1.0-Final_Core_Pack"; Path="D:\test\aionui\skill_router\v0.1.0-Final_Core_Pack"; Size=3.8},
    @{Name="v0.1.0-Final_Package"; Path="D:\test\aionui\skill_router\v0.1.0-Final_Package"; Size=97},
    @{Name="v0.1.0-Intelligent_v2"; Path="D:\test\aionui\skill_router\v0.1.0-Intelligent_v2"; Size=253},
    @{Name="v0.2.0-Shield_Edition_v3"; Path="D:\test\aionui\skill_router\v0.2.0-Shield_Edition_v3"; Size=181}
)

foreach ($v in $versions) {
    Write-Host "
=== $($v.Name) ($($v.Size) MB) ==="
    
    # 检查是否有 skills 子目录
    $skillsPath = Join-Path $v.Path "skills"
    if (Test-Path $skillsPath) {
        $skillCount = Get-ChildItem $skillsPath -Directory | Measure-Object | Select-Object -ExpandProperty Count
        Write-Host "Skills 目录: 有 ($skillCount 个技能)"
    } else {
        Write-Host "Skills 目录: 无"
    }
    
    # 检查是否有嵌套的 SkillRouter 目录
    $nested = Get-ChildItem $v.Path -Directory | Where-Object { $_.Name -like "*SkillRouter*" }
    if ($nested) {
        Write-Host "嵌套目录: 是 ($($nested.Count) 个)"
    } else {
        Write-Host "嵌套目录: 否"
    }
}
