[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$workspace = Split-Path -Parent $PSScriptRoot
$androidProject = Join-Path $workspace "src-tauri\gen\android"
$pluginProject = Join-Path $workspace "src-tauri\plugins\android-bridge\android"
$gradle = Join-Path $androidProject "gradlew.bat"
if (-not (Test-Path -LiteralPath $gradle)) {
    throw "The generated Android Gradle project is missing at '$androidProject'."
}
. (Join-Path $PSScriptRoot "configure-android-environment.ps1")
$androidEnvironment = Set-MarktreeAndroidEnvironment -RequireJdk
& $androidEnvironment.Cargo build `
    --package marktree `
    --manifest-path (Join-Path $workspace "src-tauri\Cargo.toml") `
    --target aarch64-linux-android `
    --lib
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

$tauri = Join-Path $workspace "node_modules\.bin\tauri.cmd"
if (-not (Test-Path -LiteralPath $tauri -PathType Leaf)) {
    throw "The local Tauri CLI is missing. Run npm ci first."
}
$baseDirectory = if ($env:RUNNER_TEMP) {
    $env:RUNNER_TEMP
} elseif (Test-Path -LiteralPath "D:\Tools") {
    "D:\Tools"
} else {
    [System.IO.Path]::GetTempPath()
}
$evidence = Join-Path $baseDirectory ("marktree-android-native-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $evidence -Force | Out-Null
$stdout = Join-Path $evidence "tauri.stdout.log"
$stderr = Join-Path $evidence "tauri.stderr.log"
$env:PATH = "$(Join-Path $PSScriptRoot 'android-studio-noop');$env:PATH"
$env:CI = "true"

function Stop-ProcessTree([int]$ProcessId) {
    $children = Get-CimInstance Win32_Process -Filter "ParentProcessId = $ProcessId" -ErrorAction SilentlyContinue
    foreach ($child in $children) {
        Stop-ProcessTree -ProcessId $child.ProcessId
    }
    Stop-Process -Id $ProcessId -Force -ErrorAction SilentlyContinue
}

$settings = Join-Path $androidProject "tauri.settings.gradle"
$dependencies = Join-Path $androidProject "app\tauri.build.gradle.kts"
$tauriProcess = Start-Process `
    -FilePath $tauri `
    -ArgumentList @("android", "dev", "--open", "--no-dev-server", "--no-watch") `
    -WorkingDirectory $workspace `
    -WindowStyle Hidden `
    -RedirectStandardOutput $stdout `
    -RedirectStandardError $stderr `
    -PassThru
$tauriReady = $false
try {
    $deadline = [DateTime]::UtcNow.AddMinutes(4)
    while (-not $tauriProcess.HasExited -and [DateTime]::UtcNow -lt $deadline) {
        $settingsContent = if (Test-Path -LiteralPath $settings) {
            Get-Content -LiteralPath $settings -Raw
        }
        $dependenciesContent = if (Test-Path -LiteralPath $dependencies) {
            Get-Content -LiteralPath $dependencies -Raw
        }
        $tauriOutput = @(
            Get-Content -LiteralPath $stdout -Raw -ErrorAction SilentlyContinue
            Get-Content -LiteralPath $stderr -Raw -ErrorAction SilentlyContinue
        ) -join "`n"
        if ($settingsContent -match "marktree-android-bridge" -and
            $dependenciesContent -match "marktree-android-bridge" -and
            $tauriOutput -match "Opening Android Studio") {
            $tauriReady = $true
            break
        }
        Start-Sleep -Milliseconds 500
        $tauriProcess.Refresh()
    }
} finally {
    Stop-ProcessTree -ProcessId $tauriProcess.Id
}
if (-not $tauriReady) {
    throw "Tauri did not prepare the complete Android project. Evidence: $evidence"
}

Push-Location $androidProject
try {
    & $gradle `
        :app:compileArmDebugKotlin `
        :marktree-android-bridge:compileDebugKotlin `
        :app:processArmDebugMainManifest `
        :app:compileUniversalDebugKotlin `
        :app:processUniversalDebugMainManifest `
        --no-daemon `
        --stacktrace
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
} finally {
    Pop-Location
}

$sharePluginClass = Get-ChildItem -LiteralPath (Join-Path $pluginProject "build") `
    -Filter "SharePlugin.class" -File -Recurse -ErrorAction SilentlyContinue |
    Select-Object -First 1
if (-not $sharePluginClass) {
    throw "The Android share plugin did not produce SharePlugin.class."
}
$mergedManifests = @(
    Get-ChildItem -LiteralPath (Join-Path $androidProject "app\build\intermediates\merged_manifests") `
        -Filter "AndroidManifest.xml" -File -Recurse -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match "(?:arm|universal)Debug" }
)
$validManifests = @(
    $mergedManifests | Where-Object {
        $manifest = Get-Content -LiteralPath $_.FullName -Raw
        $manifest -match "android.intent.action.SEND" -and
            $manifest -match "android.intent.action.VIEW" -and
            $manifest -match "io.github.cheshiremew.marktree.MainActivity" -and
            $manifest -match "\.fileprovider" -and
            $manifest -notmatch "\.shareprovider"
    }
)
if ($mergedManifests.Count -ne 2 -or $validManifests.Count -ne $mergedManifests.Count) {
    throw "The final Arm and Universal manifests do not expose the supported share entry points and unique FileProvider."
}
