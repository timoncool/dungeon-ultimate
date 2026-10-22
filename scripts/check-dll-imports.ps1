# Checks that every library the game's engines import is beside them, beside the game's exe (VC++ runtime from the
# installer) or in Windows: walks the import tables of an installed root (crates/du-server/src/dll_imports.rs).
# Run it on a root where First Run downloaded the components, after changing any engine or CUDA pin in setup.rs.
param(
    [Parameter(Mandatory = $true)][string]$Root
)
$ErrorActionPreference = 'Stop'
$env:DU_DLL_ROOT = (Resolve-Path $Root).Path
Push-Location (Join-Path $PSScriptRoot '..')
try {
    cargo test -p du-server --lib dll_imports::tests::an_installed_root_has_every_import -- --ignored --nocapture
    if ($LASTEXITCODE -ne 0) { throw "DLL import check failed (exit $LASTEXITCODE)" }
} finally {
    Pop-Location
}
