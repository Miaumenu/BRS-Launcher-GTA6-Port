# Starts the title with the settings this fork is played with (README.md, "How to run it").
#   scripts\play.ps1 -Exe <path of your own copy's executable>
# -Safe         without the newer, faster ways (the first thing to try when something is wrong)
# -RayTracing   the title's own ray-traced indirect light: lit interiors, about a third slower
# -Build        target\diag (the default, the build all measurements are of) or target\release
# -More         other settings: -More @{GAMBIARRA_PANEL='1'}
# Run it from a normal window, not as administrator: the driver then keeps its shader cache elsewhere.
param(
    [Parameter(Mandatory)][string]$Exe,
    [switch]$Safe,
    [switch]$RayTracing,
    [ValidateSet('diag', 'release')][string]$Build = 'diag',
    [hashtable]$More = @{}
)
$settings = @{
    GAMBIARRA_TEXTURE_NO_WAIT = '1'; GAMBIARRA_RELAY = '2'; GAMBIARRA_WIDE_BACK = '1'; GAMBIARRA_CARD_REHASH_RUNS = '1'
    GAMBIARRA_JOIN_COPIES = '1'; GAMBIARRA_INDIRECT_WINDOW = '1'; GAMBIARRA_RELAY_COPY = '1'; GAMBIARRA_CARD_STEPS = '2'
    GAMBIARRA_INDIRECT_BATCH = '2'; GAMBIARRA_MAP_THREADS = '1'; GAMBIARRA_TITLE_PRIORITIES = '0'; GAMBIARRA_CARD_AWAKE = '1'
    GAMBIARRA_SHOW_AFTER = '1'; GAMBIARRA_AMBIENT = '0.8'; GAMBIARRA_D3D12_DRED = 'light'; GAMBIARRA_SKIP_PIPELINES = 'none'
}
if ($Safe) {
    $settings.GAMBIARRA_UNDER_WIDE = '0'; $settings.GAMBIARRA_LAND_ECHO = '0'
} else {
    $settings.GAMBIARRA_THIN = '1'; $settings.GAMBIARRA_JOIN_PARTS = '8'; $settings.GAMBIARRA_UPLOAD_AHEAD = '1'
}
if ($RayTracing) { $settings.GAMBIARRA_BVH_BUILDER = '1' }
foreach ($k in $More.Keys) { $settings[$k] = $More[$k] }

$launcher = Join-Path $PSScriptRoot "..\target\$Build\gambiarra-cli.exe"
if (-not (Test-Path $launcher)) { Write-Host "No $launcher. Build it first (README.md)."; return }
if (-not (Test-Path $Exe)) { Write-Host "No $Exe."; return }
# (Other settings left over in this window go; where your files are stays.)
$paths = 'GAMBIARRA_CONFIG', 'GAMBIARRA_GAME_DIR', 'GAMBIARRA_CACHE_DIR', 'GAMBIARRA_DATA_DIR'
Get-ChildItem Env:GAMBIARRA_* | Where-Object { $_.Name -notin $paths } | ForEach-Object { Remove-Item "Env:$($_.Name)" }
foreach ($k in $settings.Keys) { Set-Item "Env:$k" $settings[$k] }
$Exe = (Resolve-Path $Exe).Path
Push-Location (Join-Path $PSScriptRoot '..')   # launcher config is read here
try { & $launcher run $Exe --execute } finally { Pop-Location }
