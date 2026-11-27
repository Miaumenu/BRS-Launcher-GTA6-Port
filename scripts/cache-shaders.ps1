param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string[]]$Path,

    [ValidateRange(0, 1024)]
    [int]$Jobs = 0,

    [string]$CliPath
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot

if ([string]::IsNullOrWhiteSpace($CliPath)) {
    $CliPath = Join-Path $repoRoot 'target\release\gambiarra-cli.exe'
}

if (-not (Test-Path -LiteralPath $CliPath -PathType Leaf)) {
    throw "CLI não encontrado em '$CliPath'. Compile gambiarra-cli ou informe -CliPath."
}

$resolvedPaths = foreach ($item in $Path) {
    $resolved = Resolve-Path -LiteralPath $item -ErrorAction Stop
    $resolved.Path
}

$arguments = @('shaders') + @($resolvedPaths)
if ($Jobs -gt 0) {
    $arguments += @('--jobs', "$Jobs")
}

& $CliPath @arguments
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
