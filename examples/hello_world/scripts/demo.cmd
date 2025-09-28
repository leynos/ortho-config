@echo off
setlocal enabledelayedexpansion

set "SCRIPT_DIR=%~dp0"
for %%I in ("%SCRIPT_DIR%..") do set "EXAMPLE_ROOT=%%~fI"
set "WORKDIR=%TEMP%\hello-world-demo-%RANDOM%%RANDOM%"
if exist "%WORKDIR%" rd /s /q "%WORKDIR%"
md "%WORKDIR%" || exit /b 1

call :prepare_config baseline.toml
if errorlevel 1 call :bail %ERRORLEVEL%
call :run "Running greet with baseline config defaults" cargo run -p hello_world --manifest-path "%EXAMPLE_ROOT%\Cargo.toml" --quiet -- greet
if errorlevel 1 call :bail %ERRORLEVEL%
call :run "Running take-leave with baseline config defaults" cargo run -p hello_world --manifest-path "%EXAMPLE_ROOT%\Cargo.toml" --quiet -- take-leave
if errorlevel 1 call :bail %ERRORLEVEL%

echo ==> Overriding recipient via HELLO_WORLD_RECIPIENT
echo     $ cargo run -p hello_world --manifest-path "%EXAMPLE_ROOT%\Cargo.toml" --quiet -- greet
pushd "%WORKDIR%" >nul
set "HELLO_WORLD_RECIPIENT=Environment override"
cargo run -p hello_world --manifest-path "%EXAMPLE_ROOT%\Cargo.toml" --quiet -- greet
set "ERR=%ERRORLEVEL%"
set "HELLO_WORLD_RECIPIENT="
popd >nul
if not "%ERR%"=="0" call :bail %ERR%
echo.

call :run "Overriding salutations via CLI arguments" cargo run -p hello_world --manifest-path "%EXAMPLE_ROOT%\Cargo.toml" --quiet -- -s "CLI hello" -r "CLI crew" greet
if errorlevel 1 call :bail %ERRORLEVEL%

call :prepare_config overrides.toml
if errorlevel 1 call :bail %ERRORLEVEL%
call :run "Running greet with overrides.toml extending baseline" cargo run -p hello_world --manifest-path "%EXAMPLE_ROOT%\Cargo.toml" --quiet -- greet
if errorlevel 1 call :bail %ERRORLEVEL%

call :cleanup
exit /b 0

:bail
set "ERR=%~1"
call :cleanup
exit /b %ERR%

:prepare_config
del /q "%WORKDIR%\.hello_world.toml" >nul 2>nul
for %%F in ("%EXAMPLE_ROOT%\config\*.toml") do (
  copy "%%~fF" "%WORKDIR%\%%~nxF" >nul
  if errorlevel 1 exit /b 1
)
copy "%WORKDIR%\%~1" "%WORKDIR%\.hello_world.toml" >nul
if errorlevel 1 exit /b 1
exit /b 0

:run
set "DESC=%~1"
shift
echo ==> %DESC%
echo     $ %*
pushd "%WORKDIR%" >nul
%*
set "ERR=%ERRORLEVEL%"
popd >nul
echo.
exit /b %ERR%

:cleanup
if exist "%WORKDIR%" rd /s /q "%WORKDIR%"
exit /b 0
