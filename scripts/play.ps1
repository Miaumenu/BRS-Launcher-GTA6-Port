# Starts the title with the settings this fork is played with (README.md, "How to run it").
#   scripts\play.ps1 -Exe <path of your own copy's executable>
# -Safe         without the newer, faster ways (the first thing to try when something is wrong)
# -RayTracing   the title's own ray-traced indirect light: lit interiors, about a third slower
# -Build        target\diag (the default, the build all measurements are of) or target\release
# -More         other settings: -More @{SIXM_PANEL='1'}
# Run it from a normal window, not as administrator: the driver then keeps its shader cache elsewhere.
param(
    [Parameter(Mandatory)][string]$Exe,
    [switch]$Safe,
    [switch]$RayTracing,
    [ValidateSet('diag', 'release')][string]$Build = 'diag',
    [hashtable]$More = @{}
)
$settings = @{
    SIXM_TEXTURE_NO_WAIT = '1'; SIXM_RELAY = '2'; SIXM_WIDE_BACK = '1'; SIXM_CARD_REHASH_RUNS = '1'
    SIXM_JOIN_COPIES = '1'; SIXM_INDIRECT_WINDOW = '1'; SIXM_RELAY_COPY = '1'; SIXM_CARD_STEPS = '2'
    SIXM_INDIRECT_BATCH = '2'; SIXM_MAP_THREADS = '1'; SIXM_TITLE_PRIORITIES = '0'; SIXM_CARD_AWAKE = '1'
    SIXM_SHOW_AFTER = '1'; SIXM_AMBIENT = '0.8'; SIXM_D3D12_DRED = 'light'; SIXM_SKIP_PIPELINES = 'none'
}
if ($Safe) {
    $settings.SIXM_UNDER_WIDE = '0'; $settings.SIXM_LAND_ECHO = '0'
} else {
    $settings.SIXM_THIN = '1'; $settings.SIXM_JOIN_PARTS = '8'; $settings.SIXM_UPLOAD_AHEAD = '1'
}
if ($RayTracing) { $settings.SIXM_BVH_BUILDER = '1' }
foreach ($k in $More.Keys) { $settings[$k] = $More[$k] }

$launcher = Join-Path $PSScriptRoot "..\target\$Build\sixm-multiversion.exe"
if (-not (Test-Path $launcher)) { Write-Host "No $launcher. Build it first (README.md)."; return }
if (-not (Test-Path $Exe)) { Write-Host "No $Exe."; return }
# (Other settings left over in this window go; where your files are stays.)
$paths = 'SIXM_CONFIG', 'SIXM_GAME_DIR', 'SIXM_CACHE_DIR', 'SIXM_DATA_DIR'
Get-ChildItem Env:SIXM_* | Where-Object { $_.Name -notin $paths } | ForEach-Object { Remove-Item "Env:$($_.Name)" }
foreach ($k in $settings.Keys) { Set-Item "Env:$k" $settings[$k] }
$Exe = (Resolve-Path $Exe).Path
Push-Location (Join-Path $PSScriptRoot '..')   # launcher config is read here
try { & $launcher run $Exe --execute } finally { Pop-Location }
