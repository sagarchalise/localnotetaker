# Per-user install (no admin): copies the app to %LOCALAPPDATA%\Programs\MyNoteTaker and adds
# Start-menu and Desktop shortcuts. Run: powershell -ExecutionPolicy Bypass -File install.ps1
$ErrorActionPreference = 'Stop'
$src = $PSScriptRoot
$dest = Join-Path $env:LOCALAPPDATA 'Programs\MyNoteTaker'
Get-Process MyNoteTaker -ErrorAction SilentlyContinue | Stop-Process -Force
New-Item -ItemType Directory -Force $dest | Out-Null
Copy-Item "$src\*" $dest -Recurse -Force -Exclude 'install.ps1'
Copy-Item "$src\uninstall.ps1" $dest -Force
$shell = New-Object -ComObject WScript.Shell
foreach ($dir in @([Environment]::GetFolderPath('Programs'), [Environment]::GetFolderPath('Desktop'))) {
  $lnk = $shell.CreateShortcut((Join-Path $dir 'MyNoteTaker.lnk'))
  $lnk.TargetPath = Join-Path $dest 'MyNoteTaker.exe'
  $lnk.WorkingDirectory = $dest
  $lnk.Save()
}
Write-Host "Installed to $dest. Start it from the Start menu or the desktop shortcut."
