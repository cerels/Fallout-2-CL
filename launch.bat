@echo off
rem Starts the Fallout 2 CE launcher. For Steam, set the game's launch options to:
rem   "C:\path\to\Fallout 2\launcher\launch.bat" %command%
rem --ui / --no-ui are passed on to the launcher. Unverified: unlike launch.sh on Linux/macOS,
rem there's no confirmed way to stop Steam also running %command% afterward - see
rem backend/launch_option.py.
setlocal enabledelayedexpansion

set "dir=%~dp0"
set "args="
:parse
if "%~1"=="" goto :afterparse
if "%~1"=="--ui" set "args=!args! --ui"
if "%~1"=="--no-ui" set "args=!args! --no-ui"
shift
goto :parse
:afterparse

where python3 >nul 2>nul
if %errorlevel%==0 (
    python3 -I "%dir%f2launcher.py" !args!
    exit /b %errorlevel%
)
where python >nul 2>nul
if %errorlevel%==0 (
    python -I "%dir%f2launcher.py" !args!
    exit /b %errorlevel%
)

rem No Python: start the game directly.
cd /d "%dir%.."
start "" /wait "fallout2-ce.exe"
