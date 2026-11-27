@echo off
setlocal EnableExtensions DisableDelayedExpansion
if not "%~1"=="" if /i not "%~1"=="pareto-residency" (
  echo Usage: SixM-MultiVersion.cmd pareto-residency 1>&2
  exit /b 2
)
if not defined SIXM_GAME_DIR set "SIXM_GAME_DIR=%~dp0..\GTA6.Xbox.v1.0.2.1"
if not exist "%SIXM_GAME_DIR%\game_scarlett_master.exe" (
  echo Set SIXM_GAME_DIR to your own dump folder containing game_scarlett_master.exe. 1>&2
  exit /b 2
)
set "runner=%~dp0sixm-multiversion-3d-rtx-pareto-residency.exe"
if not exist "%runner%" set "runner=%~dp0sixm-3d-rtx-pareto-residency.exe"
if not exist "%runner%" set "runner=%~dp0target\release\sixm-multiversion.exe"
if not exist "%runner%" (
  echo Extract the Windows release or run cargo build --release --locked -p sixm-multiversion. 1>&2
  exit /b 2
)
if not defined SIXM_AMBIENT_AIR set "SIXM_AMBIENT_AIR=1.5"
if not defined SIXM_CACHE_DIR set "SIXM_CACHE_DIR=%LOCALAPPDATA%\SixMultiVersion\cache-pareto"
if not defined SIXM_TEXTURE_NO_WAIT set "SIXM_TEXTURE_NO_WAIT=1"
if not defined SIXM_RELAY set "SIXM_RELAY=2"
if not defined SIXM_WIDE_BACK set "SIXM_WIDE_BACK=1"
if not defined SIXM_CARD_REHASH_RUNS set "SIXM_CARD_REHASH_RUNS=1"
if not defined SIXM_JOIN_COPIES set "SIXM_JOIN_COPIES=1"
if not defined SIXM_INDIRECT_WINDOW set "SIXM_INDIRECT_WINDOW=1"
if not defined SIXM_RELAY_COPY set "SIXM_RELAY_COPY=1"
if not defined SIXM_CARD_STEPS set "SIXM_CARD_STEPS=2"
if not defined SIXM_INDIRECT_BATCH set "SIXM_INDIRECT_BATCH=2"
if not defined SIXM_MAP_THREADS set "SIXM_MAP_THREADS=1"
if not defined SIXM_CARD_AWAKE set "SIXM_CARD_AWAKE=1"
if not defined SIXM_SHOW_AFTER set "SIXM_SHOW_AFTER=1"
if not defined SIXM_AMBIENT set "SIXM_AMBIENT=0.8"
if not defined SIXM_THIN set "SIXM_THIN=1"
if not defined SIXM_JOIN_PARTS set "SIXM_JOIN_PARTS=8"
if not defined SIXM_UPLOAD_AHEAD set "SIXM_UPLOAD_AHEAD=1"
if not defined SIXM_LEVELS_ON_DEMAND set "SIXM_LEVELS_ON_DEMAND=1"
if not defined SIXM_CARD_BUFFERS_MIN_KB set "SIXM_CARD_BUFFERS_MIN_KB=64"
if not defined SIXM_LOG if not defined RUST_LOG set "SIXM_LOG=warn"
"%runner%" run "%SIXM_GAME_DIR%\game_scarlett_master.exe" --console series-s --execute
exit /b %errorlevel%
