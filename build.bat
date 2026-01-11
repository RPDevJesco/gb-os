@echo off
REM =============================================================================
REM build.bat - Docker Build Script for Windows
REM =============================================================================
setlocal EnableDelayedExpansion

REM Configuration
set IMAGE_NAME=gb-os-builder
set CONTAINER_NAME=gb-os-build-container

REM Parse command
set COMMAND=%~1
if "%COMMAND%"=="" set COMMAND=release

REM Route to appropriate handler
if /i "%COMMAND%"=="release" goto BUILD_RELEASE
if /i "%COMMAND%"=="debug" goto BUILD_DEBUG
if /i "%COMMAND%"=="sdcard" goto BUILD_SDCARD
if /i "%COMMAND%"=="shell" goto RUN_SHELL
if /i "%COMMAND%"=="rebuild" goto REBUILD
if /i "%COMMAND%"=="clean" goto CLEAN
if /i "%COMMAND%"=="help" goto USAGE
if /i "%COMMAND%"=="--help" goto USAGE
if /i "%COMMAND%"=="-h" goto USAGE

echo [ERROR] Unknown command: %COMMAND%
echo.
goto USAGE

REM =============================================================================
:BUILD_RELEASE
REM =============================================================================
call :CHECK_DOCKER
if errorlevel 1 goto END
call :BUILD_IMAGE
if errorlevel 1 goto END
call :RUN_BUILD release 0
goto END

REM =============================================================================
:BUILD_DEBUG
REM =============================================================================
call :CHECK_DOCKER
if errorlevel 1 goto END
call :BUILD_IMAGE
if errorlevel 1 goto END
call :RUN_BUILD debug 0
goto END

REM =============================================================================
:BUILD_SDCARD
REM =============================================================================
call :CHECK_DOCKER
if errorlevel 1 goto END
call :BUILD_IMAGE
if errorlevel 1 goto END
call :RUN_BUILD release 1
goto END

REM =============================================================================
:RUN_SHELL
REM =============================================================================
call :CHECK_DOCKER
if errorlevel 1 goto END
call :BUILD_IMAGE
if errorlevel 1 goto END
echo [INFO] Opening interactive shell in container...
docker run --rm -it -v "%cd%:/project" --name %CONTAINER_NAME% %IMAGE_NAME% /bin/bash
goto END

REM =============================================================================
:REBUILD
REM =============================================================================
call :CHECK_DOCKER
if errorlevel 1 goto END
echo [INFO] Forcing Docker image rebuild...
docker rmi %IMAGE_NAME% 2>nul
call :BUILD_IMAGE
goto END

REM =============================================================================
:CLEAN
REM =============================================================================
echo [INFO] Cleaning up...
docker rm -f %CONTAINER_NAME% 2>nul
docker image inspect %IMAGE_NAME% >nul 2>&1
if not errorlevel 1 (
    docker rmi %IMAGE_NAME%
    echo [OK] Removed Docker image: %IMAGE_NAME%
)
if exist target rd /s /q target
if exist kernel8.img del /f kernel8.img
if exist output rd /s /q output
if exist sdcard_output rd /s /q sdcard_output
echo [OK] Clean complete
goto END

REM =============================================================================
:USAGE
REM =============================================================================
echo gb-os Bare-Metal Docker Build Script for Windows
echo.
echo Usage: build.bat [command]
echo.
echo Commands:
echo   (none)     Build release kernel
echo   debug      Build debug kernel with symbols
echo   sdcard     Build and create SD card directory with boot files
echo   shell      Open interactive shell in build container
echo   rebuild    Force rebuild of Docker image
echo   clean      Remove Docker image and build artifacts
echo   help       Show this help message
echo.
echo Examples:
echo   build.bat                  Build release kernel
echo   build.bat debug            Build debug kernel
echo   build.bat sdcard           Build and prepare SD card files
echo   build.bat shell            Interactive shell for debugging
goto END

REM =============================================================================
REM SUBROUTINES
REM =============================================================================

REM -----------------------------------------------------------------------------
:CHECK_DOCKER
REM -----------------------------------------------------------------------------
where docker >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Docker is not installed or not in PATH.
    echo Please install Docker Desktop for Windows.
    exit /b 1
)
docker info >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Docker daemon is not running.
    echo Please start Docker Desktop.
    exit /b 1
)
exit /b 0

REM -----------------------------------------------------------------------------
:BUILD_IMAGE
REM -----------------------------------------------------------------------------
docker image inspect %IMAGE_NAME% >nul 2>&1
if errorlevel 1 (
    echo [INFO] Building Docker image: %IMAGE_NAME%
    docker build -t %IMAGE_NAME% .
    if errorlevel 1 (
        echo [ERROR] Failed to build Docker image
        exit /b 1
    )
    echo [OK] Docker image built successfully
) else (
    echo [INFO] Using cached Docker image: %IMAGE_NAME%
)
exit /b 0

REM -----------------------------------------------------------------------------
:RUN_BUILD
REM -----------------------------------------------------------------------------
set BUILD_MODE=%~1
set CREATE_SDCARD=%~2

echo [INFO] Starting build (mode: %BUILD_MODE%)...

docker run --rm -v "%cd%:/project" -e "BUILD_MODE=%BUILD_MODE%" -e "CREATE_SDCARD=%CREATE_SDCARD%" --name %CONTAINER_NAME% %IMAGE_NAME%

if errorlevel 1 (
    echo [ERROR] Build failed
    exit /b 1
)

if exist "output\kernel8.img" (
    echo [OK] Build complete! Output files in .\output\
    echo.
    dir output\
) else (
    echo [ERROR] Build may have failed - output\kernel8.img not found
    exit /b 1
)
exit /b 0

REM =============================================================================
:END
REM =============================================================================
endlocal
