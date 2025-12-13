@echo off
REM Rustacean OS Docker Build Script for Windows (with GameBoy Mode)
REM
REM Usage: build.bat [options]
REM
REM Options:
REM   --gameboy     Build GameBoy edition only
REM   --both        Build both normal and GameBoy editions
REM   --rom FILE    Embed ROM file into GameBoy ISO (use with --gameboy)
REM   --tools       Build mkgamedisk tool only
REM   --no-cache    Force rebuild without Docker cache
REM   --shell       Open a shell in the build container
REM

setlocal enabledelayedexpansion

set IMAGE_NAME=rustacean-builder
set OUTPUT_DIR=%~dp0output
set NO_CACHE=
set SHELL_MODE=
set BUILD_MODE=
set ROM_FILE=

REM Parse arguments
:parse_args
if "%~1"=="" goto :done_parsing
if "%~1"=="--no-cache" (
    set NO_CACHE=--no-cache
    shift
    goto :parse_args
)
if "%~1"=="--shell" (
    set SHELL_MODE=yes
    shift
    goto :parse_args
)
if "%~1"=="--gameboy" (
    set BUILD_MODE=--gameboy
    shift
    goto :parse_args
)
if "%~1"=="--both" (
    set BUILD_MODE=--both
    shift
    goto :parse_args
)
if "%~1"=="--tools" (
    set BUILD_MODE=--tools
    shift
    goto :parse_args
)
if "%~1"=="--rom" (
    set ROM_FILE=%~2
    shift
    shift
    goto :parse_args
)
if "%~1"=="--help" goto :show_help
if "%~1"=="-h" goto :show_help
shift
goto :parse_args

:show_help
echo Rustacean OS Docker Build Script for Windows (with GameBoy Mode)
echo.
echo Usage: build.bat [options]
echo.
echo Build Options:
echo   --gameboy         Build GameBoy edition only
echo   --both            Build both normal and GameBoy editions
echo   --rom FILE        Embed ROM into GameBoy ISO (use with --gameboy)
echo   --tools           Build mkgamedisk tool only
echo   --no-cache        Force rebuild without Docker cache
echo.
echo Other Options:
echo   --shell           Open a shell in the build container
echo   --help, -h        Show this help message
echo.
echo Examples:
echo   build.bat                           Build normal Rustacean OS
echo   build.bat --gameboy                 Build GameBoy edition (no ROM)
echo   build.bat --gameboy --rom tetris.gb Build GameBoy edition WITH ROM embedded
echo   build.bat --both --rom pokemon.gb   Build both editions, GameBoy has ROM
exit /b 0

:done_parsing

REM Create output directory
if not exist "%OUTPUT_DIR%" mkdir "%OUTPUT_DIR%"

echo ========================================
echo   Rustacean OS Docker Builder
echo ========================================
echo.

REM Build Docker image
echo [Docker] Building image '%IMAGE_NAME%'...
docker build %NO_CACHE% -t %IMAGE_NAME% .
if errorlevel 1 (
    echo [Error] Docker build failed!
    exit /b 1
)

if "%SHELL_MODE%"=="yes" (
    echo.
    echo [Docker] Opening shell in container...
    docker run --rm -it -v "%OUTPUT_DIR%:/output" %IMAGE_NAME% /bin/bash
    exit /b 0
)

echo.
echo [Docker] Running build %BUILD_MODE%...

REM If ROM file specified, mount it and set ROM_FILE env
if not "%ROM_FILE%"=="" (
    for %%F in ("%ROM_FILE%") do set ROM_DIR=%%~dpF
    for %%F in ("%ROM_FILE%") do set ROM_NAME=%%~nxF
    echo [ROM] Embedding: %ROM_FILE%
    docker run --rm -v "%OUTPUT_DIR%:/output" -v "!ROM_DIR!:/input:ro" -e "ROM_FILE=/input/!ROM_NAME!" %IMAGE_NAME% /build.sh %BUILD_MODE%
) else (
    docker run --rm -v "%OUTPUT_DIR%:/output" %IMAGE_NAME% /build.sh %BUILD_MODE%
)

if errorlevel 1 (
    echo [Error] Build failed!
    exit /b 1
)

echo.
echo ========================================
echo   Output Files
echo ========================================
dir "%OUTPUT_DIR%"

echo.
echo Done! Output files are in: %OUTPUT_DIR%
echo.

if "%BUILD_MODE%"=="--gameboy" (
    echo To run GameBoy mode:
    echo   qemu-system-i386 -cdrom "%OUTPUT_DIR%\gameboy-system.iso" -boot d -m 256M
    echo.
    if not "%ROM_FILE%"=="" (
        echo ROM embedded: %ROM_FILE%
    ) else (
        echo No ROM embedded. Use --rom FILE to embed a game.
    )
) else (
    echo To run:
    echo   qemu-system-i386 -cdrom "%OUTPUT_DIR%\rustacean.iso" -boot d -m 256M
)
