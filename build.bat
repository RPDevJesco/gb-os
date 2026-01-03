@echo off
REM rustboot Windows Build Script
REM Builds all platform bootloaders using Docker

setlocal enabledelayedexpansion

echo ========================================
echo  rustboot - Docker Build
echo ========================================
echo.

REM Check if Docker is available
where docker >nul 2>nul
if %errorlevel% neq 0 (
    echo ERROR: Docker is not installed or not in PATH
    echo Please install Docker Desktop from https://docker.com
    pause
    exit /b 1
)

REM Check if Docker daemon is running
docker info >nul 2>nul
if %errorlevel% neq 0 (
    echo ERROR: Docker daemon is not running
    echo Please start Docker Desktop and try again
    pause
    exit /b 1
)

set IMAGE_NAME=rustboot-builder
set CONTAINER_NAME=rustboot-build-container

echo [1/4] Building Docker image...
echo This may take a few minutes on first run...
echo.

docker build -t %IMAGE_NAME% .
if %errorlevel% neq 0 (
    echo.
    echo ERROR: Docker image build failed
    pause
    exit /b 1
)

echo.
echo [2/4] Running build container...
echo.

REM Remove any existing container with same name
docker rm -f %CONTAINER_NAME% >nul 2>nul

REM Run the build
docker run --name %CONTAINER_NAME% %IMAGE_NAME%
if %errorlevel% neq 0 (
    echo.
    echo ERROR: Build failed inside container
    pause
    exit /b 1
)

echo.
echo [3/4] Copying output files...
echo.

REM Remove old output directory
if exist output rmdir /s /q output

REM Copy output from container
docker cp %CONTAINER_NAME%:/build/output ./output
docker cp %CONTAINER_NAME%:/build/rustboot-binaries.tar.gz ./

if %errorlevel% neq 0 (
    echo.
    echo ERROR: Failed to copy output files
    pause
    exit /b 1
)

echo.
echo [4/4] Cleaning up...
echo.

docker rm %CONTAINER_NAME% >nul 2>nul

echo.
echo ========================================
echo  Build Complete!
echo ========================================
echo.
echo Output files are in the 'output' directory:
echo.

REM List output files
for /d %%d in (output\*) do (
    echo   %%~nxd\
    for %%f in (%%d\*.img %%d\*.bin) do (
        if exist %%f (
            for %%s in (%%f) do echo     %%~nxf - %%~zs bytes
        )
    )
)

echo.
echo Combined archive: rustboot-binaries.tar.gz
echo.
pause
