[CmdletBinding()]
param(
    [string]$Target = "aarch64-linux-android",
    [int]$ApiLevel = 24,
    [string]$AndroidSdk = $(if ($env:ANDROID_HOME) { $env:ANDROID_HOME } else { "D:\Tools\Android\Sdk" })
)

$ErrorActionPreference = "Stop"

function Require-Path {
    param(
        [string]$Path,
        [string]$Description
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        throw "$Description was not found at '$Path'."
    }
    return (Resolve-Path -LiteralPath $Path).Path
}

$workspace = Split-Path -Parent $PSScriptRoot
$cargoHome = Require-Path "D:\Tools\Rust\cargo" "Cargo home"
$rustupHome = Require-Path "D:\Tools\Rust\rustup" "Rustup home"
$cargo = Require-Path (Join-Path $cargoHome "bin\cargo.exe") "Cargo"
$androidSdk = Require-Path $AndroidSdk "Android SDK"
$ndkRoot = Require-Path (Join-Path $androidSdk "ndk") "Android NDK directory"
$ndk = Get-ChildItem -LiteralPath $ndkRoot -Directory |
    Sort-Object { [version]$_.Name } -Descending |
    Select-Object -First 1
if ($null -eq $ndk) {
    throw "No Android NDK installation was found under '$ndkRoot'."
}

$toolchain = Require-Path (
    Join-Path $ndk.FullName "toolchains\llvm\prebuilt\windows-x86_64\bin"
) "Android LLVM toolchain"
$ndkTools = Require-Path (
    Join-Path $ndk.FullName "prebuilt\windows-x86_64\bin"
) "Android NDK build tools"
$gitTools = Require-Path "C:\Program Files\Git\usr\bin" "Git for Windows Unix tools"
$gitPerl = Require-Path (Join-Path $gitTools "perl.exe") "Git Perl"
$cygpath = Require-Path (Join-Path $gitTools "cygpath.exe") "Git cygpath"
$gitPerlLib = Require-Path "D:\Tools\GitPerlLib" "Git Perl support modules"

$compilerPrefix = switch ($Target) {
    "aarch64-linux-android" { "aarch64-linux-android" }
    "armv7-linux-androideabi" { "armv7a-linux-androideabi" }
    "i686-linux-android" { "i686-linux-android" }
    "x86_64-linux-android" { "x86_64-linux-android" }
    default { throw "Unsupported Android Rust target '$Target'." }
}
$compiler = Require-Path (
    Join-Path $toolchain "$compilerPrefix$ApiLevel-clang.cmd"
) "Android compiler"
$archiver = Require-Path (Join-Path $toolchain "llvm-ar.exe") "Android archiver"
$perlLibMsys = (& $cygpath -u $gitPerlLib).Trim()
if ($LASTEXITCODE -ne 0 -or -not $perlLibMsys) {
    throw "Git cygpath could not translate '$gitPerlLib'."
}

$targetEnvironment = $Target.Replace("-", "_")
$linkerEnvironment = "CARGO_TARGET_$($targetEnvironment.ToUpperInvariant())_LINKER"
$compilerEnvironment = "CC_$targetEnvironment"
$flagsEnvironment = "CFLAGS_$targetEnvironment"
$archiverEnvironment = "AR_$targetEnvironment"

$env:CARGO_HOME = $cargoHome
$env:RUSTUP_HOME = $rustupHome
$env:ANDROID_HOME = $androidSdk
$env:ANDROID_NDK_HOME = $ndk.FullName
$env:PERL5LIB = $perlLibMsys
$env:MSYS2_ENV_CONV_EXCL = "PERL5LIB"
$env:Path = "$gitTools;$ndkTools;$toolchain;$env:Path"
Set-Item -Path "Env:$linkerEnvironment" -Value $compiler.Replace("\", "/")
Set-Item -Path "Env:$compilerEnvironment" -Value "clang.exe"
Set-Item -Path "Env:$flagsEnvironment" -Value "--target=$compilerPrefix$ApiLevel"
Set-Item -Path "Env:$archiverEnvironment" -Value (Split-Path -Leaf $archiver)

& $gitPerl -MPod::Usage -MLocale::Maketext::Simple -e "exit 0"
if ($LASTEXITCODE -ne 0) {
    throw "Git Perl could not load the modules required by the vendored OpenSSL build."
}

& $cargo check `
    --manifest-path (Join-Path $workspace "src-tauri\Cargo.toml") `
    --target $Target
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
