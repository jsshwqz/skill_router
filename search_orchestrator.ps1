$path = "D:\test\aionui\forge\aion-router\src\builtins\orchestrator.rs"
$lines = Get-Content $path -Encoding UTF8
$terms = @("parse_engines", "is_high_risk", "force_triple_execute", "pub fn", "fn ", "impl ", "async fn")
$found = @()
for ($i = 0; $i -lt $lines.Count; $i++) {
    $line = $lines[$i]
    for ($t in $terms) {
        if ($line -like "*$t*") {
            $short = if ($line.Length -gt 120) { $line.Substring(0,120) } else { $line }
            $found += "L$($i+1): $short"
            break
        }
    }
}
if ($found.Count -lt 100) {
    $found | ForEach-Object { Write-Output $_ }
} else {
    Write-Output "TOTAL MATCHES: $($found.Count)"
    $found | Select-Object -First 100 | ForEach-Object { Write-Output $_ }
}
