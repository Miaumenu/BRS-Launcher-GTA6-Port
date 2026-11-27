@echo off
setlocal EnableExtensions DisableDelayedExpansion
if not "%~1"=="" if /i not "%~1"=="pareto-residency" (
  echo Usage: GambiarraLauncher.cmd pareto-residency 1>&2
  exit /b 2
)
if not defined GAMBIARRA_GAME_DIR set "GAMBIARRA_GAME_DIR=%~dp0..\GTA6.Xbox.v1.0.2.1"
if not exist "%GAMBIARRA_GAME_DIR%\game_scarlett_master.exe" (
  echo Set GAMBIARRA_GAME_DIR to your own dump folder containing game_scarlett_master.exe. 1>&2
  exit /b 2
)
set "runner=%~dp0gambiarra-cli-3d-rtx-pareto-residency.exe"
if not exist "%runner%" set "runner=%~dp0gambiarra-3d-rtx-pareto-residency.exe"
if not exist "%runner%" set "runner=%~dp0target\release\gambiarra-cli.exe"
if not exist "%runner%" (
  echo Extract the Windows release or run cargo build --release --locked -p gambiarra-cli. 1>&2
  exit /b 2
)
if not defined GAMBIARRA_AMBIENT_AIR set "GAMBIARRA_AMBIENT_AIR=1.5"
if not defined GAMBIARRA_CACHE_DIR set "GAMBIARRA_CACHE_DIR=%LOCALAPPDATA%\GambiarraLauncher\cache-pareto"
if not defined GAMBIARRA_TEXTURE_NO_WAIT set "GAMBIARRA_TEXTURE_NO_WAIT=1"
if not defined GAMBIARRA_RELAY set "GAMBIARRA_RELAY=2"
if not defined GAMBIARRA_WIDE_BACK set "GAMBIARRA_WIDE_BACK=1"
if not defined GAMBIARRA_CARD_REHASH_RUNS set "GAMBIARRA_CARD_REHASH_RUNS=1"
if not defined GAMBIARRA_JOIN_COPIES set "GAMBIARRA_JOIN_COPIES=1"
if not defined GAMBIARRA_INDIRECT_WINDOW set "GAMBIARRA_INDIRECT_WINDOW=1"
if not defined GAMBIARRA_RELAY_COPY set "GAMBIARRA_RELAY_COPY=1"
if not defined GAMBIARRA_CARD_STEPS set "GAMBIARRA_CARD_STEPS=2"
if not defined GAMBIARRA_INDIRECT_BATCH set "GAMBIARRA_INDIRECT_BATCH=2"
if not defined GAMBIARRA_MAP_THREADS set "GAMBIARRA_MAP_THREADS=1"
if not defined GAMBIARRA_CARD_AWAKE set "GAMBIARRA_CARD_AWAKE=1"
if not defined GAMBIARRA_SHOW_AFTER set "GAMBIARRA_SHOW_AFTER=1"
if not defined GAMBIARRA_AMBIENT set "GAMBIARRA_AMBIENT=0.8"
if not defined GAMBIARRA_THIN set "GAMBIARRA_THIN=1"
if not defined GAMBIARRA_JOIN_PARTS set "GAMBIARRA_JOIN_PARTS=8"
if not defined GAMBIARRA_UPLOAD_AHEAD set "GAMBIARRA_UPLOAD_AHEAD=1"
if not defined GAMBIARRA_LEVELS_ON_DEMAND set "GAMBIARRA_LEVELS_ON_DEMAND=1"
if not defined GAMBIARRA_CARD_BUFFERS_MIN_KB set "GAMBIARRA_CARD_BUFFERS_MIN_KB=64"
if not defined GAMBIARRA_LOG if not defined RUST_LOG set "GAMBIARRA_LOG=warn"
"%runner%" run "%GAMBIARRA_GAME_DIR%\game_scarlett_master.exe" --console series-s --execute
exit /b %errorlevel%
