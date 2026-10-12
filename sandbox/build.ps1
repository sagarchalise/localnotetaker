# Toolchain lives only in the container. Builds Windows .exe files into .\target on the host.
# Usage: .\sandbox\build.ps1 [-Mode build|clippy] [-Package spike0-catalog] [-Release]
param(
  [ValidateSet('build', 'clippy')][string]$Mode = 'build',
  [string]$Package,
  [switch]$Release
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$pkg = if ($Package) { "-p $Package" } else { '--workspace' }
$rel = if ($Release) { '--release' } else { '' }
$tail = if ($Mode -eq 'clippy') { '-- -D warnings' } else { '' }

wslc build --progress plain -t notetaker-build -f "$PSScriptRoot\Dockerfile" $PSScriptRoot
if ($LASTEXITCODE) { throw 'image build failed' }

wslc run --rm `
  -v "${root}:/work" `
  -v notetaker-cargo:/usr/local/cargo/registry `
  -v notetaker-xwin:/root/.cache/cargo-xwin `
  -e FOUNDRY_LOCAL_RUNTIME_VERSION=2.1.0 `
  -e CARGO_BUILD_JOBS=4 `
  notetaker-build `
  sh -c "cargo xwin $Mode --target x86_64-pc-windows-msvc $pkg $rel $tail"
if ($LASTEXITCODE) { throw "cargo xwin $Mode failed" }
if ($Mode -eq 'build') {
  # The SDK build script drops the native Foundry Local DLLs in its OUT_DIR; the exes need them beside them.
  $out = "$root\target\x86_64-pc-windows-msvc\$(if ($Release) {'release'} else {'debug'})"
  $dll = Get-ChildItem "$out\build" -Recurse -Filter foundry_local.dll -ErrorAction SilentlyContinue | Select-Object -First 1
  if ($dll) { Copy-Item "$($dll.DirectoryName)\*.dll" $out -Force }
}
Write-Host "Output: $root\target\x86_64-pc-windows-msvc\$(if ($Release) {'release'} else {'debug'})"
