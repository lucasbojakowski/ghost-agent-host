[CmdletBinding()]
param(
    [string]$PythonLauncher = 'py'
)

$ErrorActionPreference = 'Stop'
Push-Location $PSScriptRoot
try {
    & $PythonLauncher -3.12 -c "import sys; assert sys.version_info[:2] == (3, 12), sys.version"
    if ($LASTEXITCODE -ne 0) {
        throw 'Python 3.12 is required to build the FL Studio cp312 probe.'
    }

    & $PythonLauncher -3.12 -c "import setuptools"
    if ($LASTEXITCODE -ne 0) {
        throw 'setuptools is required. Install it with: py -3.12 -m pip install setuptools'
    }

    & $PythonLauncher -3.12 setup.py build_ext --inplace
    if ($LASTEXITCODE -ne 0) {
        throw 'Native extension build failed.'
    }

    $artifact = Get-ChildItem -LiteralPath $PSScriptRoot -Filter 'ghost_native*.pyd' |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1

    if ($null -eq $artifact) {
        throw 'Build completed but no ghost_native .pyd artifact was found.'
    }

    Write-Host "Built: $($artifact.FullName)"
}
finally {
    Pop-Location
}
