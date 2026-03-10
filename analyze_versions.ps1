# 分析每个版本的 Cargo.toml 和 README 来确定真实版本号

$versions = @(
    @{Name="v0.1.0-Final_Core_Pack"; Path="D:\test\aionui\skill_router\v0.1.0-Final_Core_Pack"},
    @{Name="v0.1.0-Final_Package"; Path="D:\test\aionui\skill_router\v0.1.0-Final_Package"},
    @{Name="v0.1.0-Intelligent_v2"; Path="D:\test\aionui\skill_router\v0.1.0-Intelligent_v2"}
)

foreach ($v in $versions) {
    Write-Host "
=== $($v.Name) ==="
    $cargo = Join-Path $v.Path "Cargo.toml"
    $readme = Join-Path $v.Path "README.md"
    
    if (Test-Path $cargo) {
        $version = Select-String -Path $cargo -Pattern '^version = "(.+)"' | Select-Object -First 1
        Write-Host "Cargo版本: $($version.Matches[0].Groups[1].Value)"
    }
    
    if (Test-Path $readme) {
        $firstLines = Get-Content $readme | Select-Object -First 5
        $desc = $firstLines | Select-String -Pattern "版本|Version|v\d" | Select-Object -First 1
        if ($desc) {
            Write-Host "描述: $($desc.Line.Trim())"
        }
    }
    
    $modules = Get-ChildItem (Join-Path $v.Path "src") -ErrorAction SilentlyContinue | Measure-Object | Select-Object -ExpandProperty Count
    Write-Host "模块数: $modules"
}
