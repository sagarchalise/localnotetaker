# Host build without Visual Studio or admin rights.
# One-time setup (already done on this machine, see sandbox/README.md):
#   - rustup (winget Rustlang.Rustup)
#   - .tools\bin\lld-link.exe = copy of rustup's rust-lld.exe
#   - .tools\xwin-splat = MSVC CRT + Windows SDK libs fetched with `xwin splat` (the symlink step
#     for the SDK headers fails without privileges; headers are not needed to link Rust code)
# Usage: .\sandbox\build-host.ps1 [-Release] [-Package mynotetaker] [-Mode build|clippy]
param(
  [ValidateSet('build', 'clippy')][string]$Mode = 'build',
  [string]$Package = 'mynotetaker',
  [switch]$Release
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$splat = "$root\.tools\xwin-splat"
$env:Path = "$root\.tools\bin;$env:USERPROFILE\.cargo\bin;$env:Path"
$env:FOUNDRY_LOCAL_RUNTIME_VERSION = '2.1.0'
$env:CARGO_TARGET_DIR = "$root\target-host"
$env:RUSTFLAGS = "-C linker=lld-link -Lnative=$splat\crt\lib\x86_64 -Lnative=$splat\sdk\lib\um\x86_64 -Lnative=$splat\sdk\lib\ucrt\x86_64"
$rel = if ($Release) { '--release' } else { '' }
$tail = if ($Mode -eq 'clippy') { '-- -D warnings' } else { '' }
Set-Location $root
cargo $Mode -p $Package $rel $tail
if ($LASTEXITCODE) { throw "cargo $Mode failed" }
if ($Mode -eq 'build') {
  $out = "$root\target-host\$(if ($Release) {'release'} else {'debug'})"
  $dll = Get-ChildItem "$out\build" -Recurse -Filter foundry_local.dll -ErrorAction SilentlyContinue | Select-Object -First 1
  if ($dll) { Copy-Item "$($dll.DirectoryName)\*.dll" $out -Force }
  Write-Host "Output: $out"
}
