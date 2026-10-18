@echo off
rem Starts the Fallout 2 CE launcher. For Steam, set the game's launch options to:
rem   "C:\path\to\Fallout 2\launcher\launch.bat" %command%
rem --ui / --no-ui / --campaign=<id> are passed on to the launcher - the last one is how
rem a different Steam entry (e.g. Fallout 1's) forces a specific campaign; see
rem backend/launch_option.py. Unverified: unlike launch.sh on Linux/macOS, there's no
rem confirmed way to stop Steam also running %command% afterward.
setlocal enabledelayedexpansion

set "dir=%~dp0"
set "args="
:parse
if "%~1"=="" goto :afterparse
set "arg=%~1"
if "%~1"=="--ui" set "args=!args! --ui"
if "%~1"=="--no-ui" set "args=!args! --no-ui"
if "!arg:~0,11!"=="--campaign=" set "args=!args! !arg!"
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
