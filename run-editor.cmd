@echo off
rem Double-clickable / cmd.exe launcher for run-editor.ps1 (bypasses the
rem PowerShell execution policy for this one script). Arguments pass through,
rem e.g.  run-editor.cmd -Release
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0run-editor.ps1" %*
exit /b %ERRORLEVEL%
