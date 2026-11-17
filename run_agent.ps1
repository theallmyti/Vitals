Write-Host "Running Vitals PC Monitor Agent..."
if (Test-Path ".\vitals.exe") {
    Start-Process ".\vitals.exe"
} elseif (Test-Path ".\pc-monitor-agent.exe") {
    Start-Process ".\pc-monitor-agent.exe"
} else {
    Set-Location "agent"
    cargo run --release
}
