@echo off
REM gb-os Docker Build Script for Windows
REM
REM Usage: build.bat [options]
REM
REM Options:
REM   --gameboy     Build GameBoy edition only (default)
REM   --normal      Build normal edition only
REM   --both        Build both normal and GameBoy editions
REM   --rom FILE    Embed ROM file into GameBoy ISO
REM   --tools       Build mkgamedisk tool only
REM   --no-cache    Force rebuild without Docker cache
REM   --shell       Open a shell in the build container
REM

setlocal enabledelayedexpansion

REM Configuration
set IMAGE_NAME=gb-os-builder
set SCRIPT_DIR=%~dp0
set SCRIPT_DIR=%SCRIPT_DIR:~0,-1%
set OUTPUT_DIR=%SCRIPT_DIR%\output
set NO_CACHE=
set SHELL_MODE=
set BUILD_MODE=--gameboy
set ROM_FILE=

REM Parse arguments
:parse_args
if "%~1"=="" goto :done_parsing
if /i "%~1"=="--no-cache" (
    set NO_CACHE=--no-cache
    shift
    goto :parse_args
)
if /i "%~1"=="--shell" (
    set SHELL_MODE=yes
    shift
    goto :parse_args
)
if /i "%~1"=="--gameboy" (
    set BUILD_MODE=--gameboy
    shift
    goto :parse_args
)
if /i "%~1"=="--normal" (
    set BUILD_MODE=--normal
    shift
    goto :parse_args
)
if /i "%~1"=="--both" (
    set BUILD_MODE=--both
    shift
    goto :parse_args
)
if /i "%~1"=="--tools" (
    set BUILD_MODE=--tools
    shift
    goto :parse_args
)
if /i "%~1"=="--rom" (
    set ROM_FILE=%~2
    shift
    shift
    goto :parse_args
)
if /i "%~1"=="--help" goto :show_help
if /i "%~1"=="-h" goto :show_help
echo [WARN] Unknown option: %~1
shift
goto :parse_args

:show_help
echo gb-os Docker Build Script for Windows
echo.
echo Usage: build.bat [options]
echo.
echo Build Options:
echo   --gameboy         Build GameBoy edition only (default)
echo   --normal          Build normal edition only
echo   --both            Build both normal and GameBoy editions
echo   --rom FILE        Embed ROM into GameBoy ISO
echo   --tools           Build mkgamedisk tool only
echo.
echo Docker Options:
echo   --no-cache        Force rebuild without Docker cache
echo   --shell           Open a shell in the build container
echo.
echo Other Options:
echo   --help, -h        Show this help message
echo.
echo Boot Methods:
echo   The built images support:
echo   - Floppy disk boot (gameboy-system.img)
echo   - CD-ROM boot with no-emulation El Torito (gameboy-system.iso)
echo   - USB/HDD boot (write gameboy-system.img to drive)
echo.
echo Examples:
echo   build.bat                           Build GameBoy edition
echo   build.bat --rom tetris.gb           Build with embedded ROM
echo   build.bat --both --rom pokemon.gb   Build both, GameBoy has ROM
echo   build.bat --shell                   Debug in container
exit /b 0

:done_parsing

REM Create output directory
if not exist "%OUTPUT_DIR%" mkdir "%OUTPUT_DIR%"

echo ========================================
echo   gb-os Docker Builder
echo   No-Emulation Boot Support
echo ========================================
echo.

REM Check Docker is available
docker --version >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Docker is not installed or not in PATH
    exit /b 1
)

REM Build Docker image
echo [INFO] Building Docker image '%IMAGE_NAME%'...
docker build %NO_CACHE% -t %IMAGE_NAME% "%SCRIPT_DIR%"
if errorlevel 1 (
    echo [ERROR] Docker build failed!
    exit /b 1
)
echo [OK] Docker image built
echo.

REM Shell mode
if "%SHELL_MODE%"=="yes" (
    echo [INFO] Opening shell in container...
    docker run --rm -it -v "%OUTPUT_DIR%:/output" %IMAGE_NAME% /bin/bash
    exit /b 0
)

REM Run the build
echo [INFO] Running build %BUILD_MODE%...

if not "%ROM_FILE%"=="" (
    REM Validate ROM file exists
    if not exist "%ROM_FILE%" (
        echo [ERROR] ROM file not found: %ROM_FILE%
        exit /b 1
    )

    REM Get absolute path and separate directory/filename
    for %%F in ("%ROM_FILE%") do (
        set ROM_DIR=%%~dpF
        set ROM_NAME=%%~nxF
    )
    REM Remove trailing backslash from ROM_DIR
    if "!ROM_DIR:~-1!"=="\" set ROM_DIR=!ROM_DIR:~0,-1!

    echo [INFO] Embedding ROM: !ROM_NAME!
    docker run --rm -v "%OUTPUT_DIR%:/output" -v "!ROM_DIR!:/input:ro" -e "ROM_FILE=/input/!ROM_NAME!" %IMAGE_NAME% /build.sh %BUILD_MODE%
) else (
    docker run --rm -v "%OUTPUT_DIR%:/output" %IMAGE_NAME% /build.sh %BUILD_MODE%
)

if errorlevel 1 (
    echo [ERROR] Build failed!
    exit /b 1
)

echo.
echo ========================================
echo   Output Files
echo ========================================
dir "%OUTPUT_DIR%" /b

echo.
echo [OK] Build complete! Output in: %OUTPUT_DIR%
echo.

echo To run GameBoy mode:
echo.
echo   Floppy boot:
echo     qemu-system-i386 -fda "%OUTPUT_DIR%\gameboy-system.img" -boot a -m 256M
echo.
echo   CD-ROM boot (no-emulation):
echo     qemu-system-i386 -cdrom "%OUTPUT_DIR%\gameboy-system.iso" -boot d -m 256M
echo.

exit /b 0
