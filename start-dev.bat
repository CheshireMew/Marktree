@echo off
setlocal

cd /d "%~dp0"
if errorlevel 1 (
  echo [Marktree] Cannot open the project directory: %~dp0
  pause
  exit /b 1
)

if exist "D:\Tools\NodeJS\node.exe" set "PATH=D:\Tools\NodeJS;%PATH%"
if exist "D:\Tools\Rust\cargo\bin\cargo.exe" (
  set "CARGO_HOME=D:\Tools\Rust\cargo"
  set "RUSTUP_HOME=D:\Tools\Rust\rustup"
  set "PATH=D:\Tools\Rust\cargo\bin;%PATH%"
)

where node.exe >nul 2>&1
if errorlevel 1 goto :missing_node

where npm.cmd >nul 2>&1
if errorlevel 1 goto :missing_npm

where cargo.exe >nul 2>&1
if errorlevel 1 goto :missing_rust

if not exist "package.json" (
  echo [Marktree] package.json was not found in the project directory.
  pause
  exit /b 1
)

if not exist "node_modules\.bin\tauri.cmd" goto :install_dependencies
if not exist "node_modules\.bin\vite.cmd" goto :install_dependencies
goto :dependencies_ready

:install_dependencies
if not exist "package-lock.json" goto :missing_package_lock
echo [Marktree] Installing dependencies from package-lock.json...
set "npm_config_cache=D:\Tools\npm-cache"
call npm.cmd ci
if errorlevel 1 goto :install_failed

:dependencies_ready

echo [Marktree] Starting the Windows desktop development app...
call npm.cmd run desktop -- %*
set "MARKTREE_EXIT_CODE=%ERRORLEVEL%"

if not "%MARKTREE_EXIT_CODE%"=="0" (
  echo.
  echo [Marktree] Startup failed with exit code %MARKTREE_EXIT_CODE%.
  pause
)

exit /b %MARKTREE_EXIT_CODE%

:missing_node
echo [Marktree] Node.js was not found. Install Node.js 24 or place it in D:\Tools\NodeJS.
pause
exit /b 1

:missing_npm
echo [Marktree] npm.cmd was not found. Check the Node.js installation.
pause
exit /b 1

:missing_rust
echo [Marktree] Cargo was not found. Install Rust stable or place it in D:\Tools\Rust.
pause
exit /b 1

:missing_package_lock
echo [Marktree] package-lock.json was not found, so dependencies cannot be installed reproducibly.
pause
exit /b 1

:install_failed
echo.
echo [Marktree] Dependency installation failed.
pause
exit /b 1
