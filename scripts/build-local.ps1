# Local release build, deployed to a fixed folder the desktop shortcut points at:
#   %LOCALAPPDATA%\Programs\MyNoteTaker\MyNoteTaker.exe (same place installer\install.ps1 uses)
# The shortcut never points into target\, so cargo clean or a new build can't break it.
# Run after any change: powershell -ExecutionPolicy Bypass -File scripts\build-local.ps1
# Uses the normal MSVC toolchain when installed (VS Build Tools, C++ workload); otherwise the xwin SDK in .tools
# with LLVM (clang-cl) and lld-link (winget install LLVM.LLVM).
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root
$env:FOUNDRY_LOCAL_RUNTIME_VERSION = '2.1.0'
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"

# cargo locates an installed MSVC toolchain itself, so link.exe needn't be on PATH.
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$hasMsvc = (Get-Command link.exe -ErrorAction SilentlyContinue) -or ((Test-Path $vswhere) -and
  (& $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath))
if (-not $hasMsvc) {
  $splat = Join-Path $root '.tools\xwin-splat'
  $llvm = 'C:\Program Files\LLVM\bin'
  if (-not (Test-Path $splat) -or -not (Test-Path $llvm)) {
    throw "No MSVC linker found, and .tools\xwin-splat or LLVM is missing."
  }
  $env:PATH = "$llvm;$root\.tools\bin;$env:PATH"
  $env:LIB = "$splat\crt\lib\x86_64;$splat\sdk\lib\ucrt\x86_64;$splat\sdk\lib\um\x86_64"
  $inc = "/imsvc $splat\crt\include /imsvc $splat\sdk\include\ucrt /imsvc $splat\sdk\include\um /imsvc $splat\sdk\include\shared"
  $env:CC_x86_64_pc_windows_msvc = 'clang-cl'
  $env:CXX_x86_64_pc_windows_msvc = 'clang-cl'
  $env:AR_x86_64_pc_windows_msvc = 'llvm-lib'
  $env:CFLAGS_x86_64_pc_windows_msvc = $inc
  $env:CXXFLAGS_x86_64_pc_windows_msvc = $inc
  $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = 'lld-link'
}

# A running copy locks the exe, and killing it could cut off a recording in progress.
if (Get-Process MyNoteTaker -ErrorAction SilentlyContinue) {
  throw "MyNoteTaker is running. Close it (let any recording finish) and run this again."
}

cargo build --release -p mynotetaker --target x86_64-pc-windows-msvc --target-dir "$root\target"
if ($LASTEXITCODE -ne 0) { throw "cargo build failed ($LASTEXITCODE)" }

# The Foundry native DLLs must sit next to the exe; take them from the newest build output.
$release = "$root\target\x86_64-pc-windows-msvc\release"
$out = Get-ChildItem "$release\build" -Directory -Filter 'foundry-local-sdk-*' |
  ForEach-Object { Join-Path $_.FullName 'out' } |
  Where-Object { Test-Path "$_\foundry_local.dll" } |
  Sort-Object { (Get-Item "$_\foundry_local.dll").LastWriteTime } -Descending |
  Select-Object -First 1
if ($out) { Copy-Item "$out\*.dll" $release -Force }

# Deploy over the previous build in place, then (re)write the shortcuts so they always exist
# and point at the same exe.
$dest = Join-Path $env:LOCALAPPDATA 'Programs\MyNoteTaker'
New-Item -ItemType Directory -Force $dest | Out-Null
Copy-Item "$release\MyNoteTaker.exe", "$release\*.dll" $dest -Force
Copy-Item "$root\installer\uninstall.ps1" $dest -Force
$exe = Join-Path $dest 'MyNoteTaker.exe'
$shell = New-Object -ComObject WScript.Shell
foreach ($dir in @([Environment]::GetFolderPath('Programs'), [Environment]::GetFolderPath('Desktop'))) {
  $lnk = $shell.CreateShortcut((Join-Path $dir 'MyNoteTaker.lnk'))
  $lnk.TargetPath = $exe
  $lnk.WorkingDirectory = $dest
  $lnk.IconLocation = "$exe,0"
  $lnk.Save()
}
Get-Item $exe | Select-Object FullName, Length, LastWriteTime
