function Set-MarktreeAndroidEnvironment {
    [CmdletBinding()]
    param(
        [string]$Target = "aarch64-linux-android",
        [int]$ApiLevel = 24,
        [string]$AndroidSdk = $(if ($env:ANDROID_HOME) { $env:ANDROID_HOME } else { "D:\Tools\Android\Sdk" }),
        [string]$CargoHome = $(if ($env:CARGO_HOME) { $env:CARGO_HOME } else { "D:\Tools\Rust\cargo" }),
        [string]$RustupHome = $(if ($env:RUSTUP_HOME) { $env:RUSTUP_HOME } else { "D:\Tools\Rust\rustup" }),
        [string]$GitPerlLib = $(if ($env:MARKTREE_GIT_PERL_LIB) { $env:MARKTREE_GIT_PERL_LIB } elseif (Test-Path -LiteralPath "D:\Tools\GitPerlLib") { "D:\Tools\GitPerlLib" } else { "" }),
        [switch]$RequireJdk
    )

    function Resolve-RequiredPath {
        param([string]$Path, [string]$Description)
        if (-not (Test-Path -LiteralPath $Path)) {
            throw "$Description was not found at '$Path'."
        }
        return (Resolve-Path -LiteralPath $Path).Path
    }

    $cargoHome = Resolve-RequiredPath $CargoHome "Cargo home"
    $rustupHome = Resolve-RequiredPath $RustupHome "Rustup home"
    $cargo = Resolve-RequiredPath (Join-Path $cargoHome "bin\cargo.exe") "Cargo"
    $rustup = Resolve-RequiredPath (Join-Path $cargoHome "bin\rustup.exe") "Rustup"
    $androidSdk = Resolve-RequiredPath $AndroidSdk "Android SDK"
    $ndkRoot = Resolve-RequiredPath (Join-Path $androidSdk "ndk") "Android NDK directory"
    $ndk = Get-ChildItem -LiteralPath $ndkRoot -Directory |
        Sort-Object { [version]$_.Name } -Descending |
        Select-Object -First 1
    if (-not $ndk) {
        throw "No Android NDK installation was found under '$ndkRoot'."
    }

    $toolchain = Resolve-RequiredPath (
        Join-Path $ndk.FullName "toolchains\llvm\prebuilt\windows-x86_64\bin"
    ) "Android LLVM toolchain"
    $ndkTools = Resolve-RequiredPath (
        Join-Path $ndk.FullName "prebuilt\windows-x86_64\bin"
    ) "Android NDK build tools"
    $git = Get-Command git.exe -ErrorAction Stop
    $gitInstallation = Split-Path -Parent (Split-Path -Parent $git.Source)
    $gitTools = Resolve-RequiredPath (Join-Path $gitInstallation "usr\bin") "Git for Windows Unix tools"
    $gitPerl = Resolve-RequiredPath (Join-Path $gitTools "perl.exe") "Git Perl"

    $compilerPrefix = switch ($Target) {
        "aarch64-linux-android" { "aarch64-linux-android" }
        "armv7-linux-androideabi" { "armv7a-linux-androideabi" }
        "i686-linux-android" { "i686-linux-android" }
        "x86_64-linux-android" { "x86_64-linux-android" }
        default { throw "Unsupported Android Rust target '$Target'." }
    }
    $compiler = Resolve-RequiredPath (
        Join-Path $toolchain "$compilerPrefix$ApiLevel-clang.cmd"
    ) "Android compiler"
    $cppCompiler = Resolve-RequiredPath (
        Join-Path $toolchain "$compilerPrefix$ApiLevel-clang++.cmd"
    ) "Android C++ compiler"
    $archiver = Resolve-RequiredPath (Join-Path $toolchain "llvm-ar.exe") "Android archiver"
    $targetEnvironment = $Target.Replace("-", "_")

    $env:CARGO_HOME = $cargoHome
    $env:RUSTUP_HOME = $rustupHome
    $env:ANDROID_HOME = $androidSdk
    $env:ANDROID_NDK_HOME = $ndk.FullName
    $env:NDK_HOME = $ndk.FullName
    $env:ANDROID_NATIVE_API_LEVEL = $ApiLevel.ToString()
    $env:TARGET_AR = $archiver
    $env:TARGET_CC = $compiler
    $env:TARGET_CXX = $cppCompiler
    if (-not $env:GRADLE_USER_HOME -and (Test-Path -LiteralPath "D:\Tools\Gradle")) {
        $env:GRADLE_USER_HOME = "D:\Tools\Gradle"
    }
    if ($GitPerlLib) {
        $cygpath = Resolve-RequiredPath (Join-Path $gitTools "cygpath.exe") "Git cygpath"
        $gitPerlLib = Resolve-RequiredPath $GitPerlLib "Git Perl support modules"
        $perlLibMsys = (& $cygpath -u $gitPerlLib).Trim()
        if ($LASTEXITCODE -ne 0 -or -not $perlLibMsys) {
            throw "Git cygpath could not translate '$gitPerlLib'."
        }
        $env:PERL5LIB = $perlLibMsys
        $env:MSYS2_ENV_CONV_EXCL = "PERL5LIB"
    }
    $env:PATH = "$(Join-Path $cargoHome 'bin');$gitTools;$ndkTools;$toolchain;$env:PATH"
    Set-Item -Path "Env:CARGO_TARGET_$($targetEnvironment.ToUpperInvariant())_LINKER" -Value $compiler.Replace("\", "/")
    Set-Item -Path "Env:CARGO_TARGET_$($targetEnvironment.ToUpperInvariant())_RUSTFLAGS" -Value "-Clink-arg=-landroid -Clink-arg=-llog -Clink-arg=-lOpenSLES"
    Set-Item -Path "Env:CC_$targetEnvironment" -Value "clang.exe"
    Set-Item -Path "Env:CFLAGS_$targetEnvironment" -Value "--target=$compilerPrefix$ApiLevel"
    Set-Item -Path "Env:AR_$targetEnvironment" -Value (Split-Path -Leaf $archiver)

    & $gitPerl -MPod::Usage -MLocale::Maketext::Simple -e "exit 0"
    if ($LASTEXITCODE -ne 0) {
        throw "Git Perl could not load the modules required by the vendored OpenSSL build."
    }
    $installedTargets = & $rustup target list --installed
    if ($LASTEXITCODE -ne 0 -or $installedTargets -notcontains $Target) {
        throw "Rust target '$Target' is not installed. Run 'rustup target add $Target' first."
    }

    if ($RequireJdk) {
        $javaCompiler = if ($env:JAVA_HOME) {
            $candidate = Join-Path $env:JAVA_HOME "bin\javac.exe"
            if (Test-Path -LiteralPath $candidate -PathType Leaf) {
                Get-Item -LiteralPath $candidate
            }
        }
        if (-not $javaCompiler) {
            foreach ($javaRoot in @("D:\Tools\Java", "D:\Java")) {
                if (-not (Test-Path -LiteralPath $javaRoot -PathType Container)) {
                    continue
                }
                $javaCompiler = Get-ChildItem -LiteralPath $javaRoot -Filter javac.exe -File -Recurse |
                    Sort-Object FullName -Descending |
                    Select-Object -First 1
                if ($javaCompiler) {
                    break
                }
            }
        }
        if (-not $javaCompiler) {
            throw "A JDK with javac is required. JAVA_HOME currently points to '$env:JAVA_HOME'."
        }
        $env:JAVA_HOME = Split-Path -Parent (Split-Path -Parent $javaCompiler.FullName)
        $env:PATH = "$(Join-Path $env:JAVA_HOME 'bin');$env:PATH"
    }

    [pscustomobject]@{
        Cargo = $cargo
        Rustup = $rustup
        AndroidSdk = $androidSdk
        Ndk = $ndk.FullName
        Target = $Target
    }
}
