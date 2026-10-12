# Package the release build into dist\MyNoteTaker (+ zip): exe, native DLLs, installer script.
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$rel = "$root\target\x86_64-pc-windows-msvc\release"
$dist = "$root\dist\MyNoteTaker"
if (-not (Test-Path "$rel\MyNoteTaker.exe")) { throw "Build first: sandbox\build.ps1 -Package mynotetaker -Release" }
Remove-Item "$root\dist" -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $dist | Out-Null
Copy-Item "$rel\MyNoteTaker.exe" $dist
Copy-Item "$rel\*.dll" $dist
Copy-Item "$PSScriptRoot\install.ps1" $dist
Copy-Item "$PSScriptRoot\uninstall.ps1" $dist
@'
MyNoteTaker - local meeting note-taker (everything runs on this PC).

Run:      double-click MyNoteTaker.exe (portable), or run install.ps1 for a Start-menu shortcut.
Record:   Start button, or Ctrl+Alt+N. Closing the window while recording saves and summarizes first.
Notes:    Documents\MyNoteTaker\YYYY-MM-DD_HHMM.md (folder changeable in the window).
Config:   %APPDATA%\MyNoteTaker\config.toml     Log: %APPDATA%\MyNoteTaker\MyNoteTaker.log
Models:   downloaded from Foundry Local on first use (speech ~700 MB, chat 330 MB - 1 GB).
'@ | Set-Content "$dist\README.txt"
Compress-Archive -Path $dist -DestinationPath "$root\dist\MyNoteTaker.zip" -Force
Get-ChildItem $dist | Select-Object Name, @{n='MB';e={[math]::Round($_.Length/1MB,1)}}
"zip: $([math]::Round((Get-Item "$root\dist\MyNoteTaker.zip").Length/1MB,1)) MB"
