@echo off
echo ========================================================
echo   Building Antigravity Account Switcher for Windows
echo ========================================================
cd /d "%~dp0\.."

echo [1/3] Installing NPM dependencies...
call npm install
if %ERRORLEVEL% NEQ 0 (
    echo Error during npm install.
    exit /b %ERRORLEVEL%
)

echo [2/3] Building Web Assets...
call npm run build
if %ERRORLEVEL% NEQ 0 (
    echo Error during npm run build.
    exit /b %ERRORLEVEL%
)

echo [3/3] Compiling Tauri 2 Native Windows Executable...
call npx tauri build --no-bundle
if %ERRORLEVEL% NEQ 0 (
    echo Error during tauri build.
    exit /b %ERRORLEVEL%
)

if not exist "bin\windows" mkdir "bin\windows"
copy /y "src-tauri\target\release\antigravity-account-switcher.exe" "bin\windows\"

echo ========================================================
echo   Build Successful!
echo   Executable located at: bin\windows\antigravity-account-switcher.exe
echo ========================================================
pause
