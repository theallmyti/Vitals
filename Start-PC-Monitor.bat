@echo off
echo Starting Vitals PC Monitor Agent...
if exist "vitals.exe" (
    start "" "vitals.exe"
) else if exist "pc-monitor-agent.exe" (
    start "" "pc-monitor-agent.exe"
) else (
    echo Binary not found. Please ensure vitals.exe is in this directory or run via cargo run.
    pause
)
