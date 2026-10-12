# Removes the app and its shortcuts. Your notes and %APPDATA%\MyNoteTaker config are kept.
$dest = Join-Path $env:LOCALAPPDATA 'Programs\MyNoteTaker'
Get-Process MyNoteTaker -ErrorAction SilentlyContinue | Stop-Process -Force
foreach ($dir in @([Environment]::GetFolderPath('Programs'), [Environment]::GetFolderPath('Desktop'))) {
  Remove-Item (Join-Path $dir 'MyNoteTaker.lnk') -ErrorAction SilentlyContinue
}
Remove-Item $dest -Recurse -Force -ErrorAction SilentlyContinue
Write-Host 'MyNoteTaker removed (notes and config kept).'
