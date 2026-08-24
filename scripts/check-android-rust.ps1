[CmdletBinding()]
param(
    [string]$Target = "aarch64-linux-android",
    [int]$ApiLevel = 24,
    [string]$AndroidSdk = $(if ($env:ANDROID_HOME) { $env:ANDROID_HOME } else { "D:\Tools\Android\Sdk" }),
    [string]$CargoHome = $(if ($env:CARGO_HOME) { $env:CARGO_HOME } else { "D:\Tools\Rust\cargo" }),
    [string]$RustupHome = $(if ($env:RUSTUP_HOME) { $env:RUSTUP_HOME } else { "D:\Tools\Rust\rustup" }),
    [string]$GitPerlLib = $(if ($env:MARKTREE_GIT_PERL_LIB) { $env:MARKTREE_GIT_PERL_LIB } elseif (Test-Path -LiteralPath "D:\Tools\GitPerlLib") { "D:\Tools\GitPerlLib" } else { "" })
)

$ErrorActionPreference = "Stop"
$workspace = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot "configure-android-environment.ps1")
$androidEnvironment = Set-MarktreeAndroidEnvironment `
    -Target $Target `
    -ApiLevel $ApiLevel `
    -AndroidSdk $AndroidSdk `
    -CargoHome $CargoHome `
    -RustupHome $RustupHome `
    -GitPerlLib $GitPerlLib

& $androidEnvironment.Cargo build `
    --package marktree `
    --manifest-path (Join-Path $workspace "src-tauri\Cargo.toml") `
    --target $Target `
    --lib
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
