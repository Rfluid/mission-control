@echo off
rem Entry point for cmd and PowerShell on Windows: runs bin\mission-control.ps1 (see there).
powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0mission-control.ps1" %*
