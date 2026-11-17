@echo off
echo Stopping Vitals PC Monitor Agent...
taskkill /F /IM vitals.exe 2>nul
taskkill /F /IM pc-monitor-agent.exe 2>nul
echo Done.
pause
