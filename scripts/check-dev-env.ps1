$ErrorActionPreference = 'Continue'

$checks = @(
    @{ Name = 'rustup'; Command = 'rustup'; Args = @('--version') },
    @{ Name = 'rustc'; Command = 'rustc'; Args = @('--version') },
    @{ Name = 'cargo'; Command = 'cargo'; Args = @('--version') },
    @{ Name = 'MSVC compiler'; Command = 'cl'; Args = @() }
)

$missing = [System.Collections.Generic.List[string]]::new()
foreach ($check in $checks) {
    $resolved = Get-Command $check.Command -ErrorAction SilentlyContinue
    if (-not $resolved) {
        Write-Output ("MISSING: {0}" -f $check.Name)
        $missing.Add($check.Name)
        continue
    }

    if ($check.Args.Count -gt 0) {
        & $resolved.Source @($check.Args) 2>&1 | Select-Object -First 1
    } else {
        Write-Output ("FOUND: {0} at {1}" -f $check.Name, $resolved.Source)
    }
}

if ($missing.Count -gt 0) {
    Write-Output ("Missing prerequisites: {0}" -f ($missing -join ', '))
    exit 1
}

Write-Output 'Development tools are present. The repository-pinned Rust toolchain is selected by rust-toolchain.toml.'
