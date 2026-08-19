[CmdletBinding()]
param(
    [string]$PythonLauncher = 'py'
)

$ErrorActionPreference = 'Stop'
$pythonArgs = @()
if ([System.IO.Path]::GetFileNameWithoutExtension($PythonLauncher) -eq 'py') {
    $pythonArgs += '-3.12'
}

Push-Location $PSScriptRoot
try {
    & $PythonLauncher @pythonArgs -c "import sys; assert sys.version_info[:2] == (3, 12), sys.version"
    if ($LASTEXITCODE -ne 0) {
        throw 'Python 3.12 is required to build the FL Studio cp312 extension.'
    }

    & $PythonLauncher @pythonArgs -c "import setuptools"
    if ($LASTEXITCODE -ne 0) {
        throw 'setuptools is required. Install it for the selected Python 3.12 interpreter.'
    }

    & $PythonLauncher @pythonArgs setup.py build_ext --inplace
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
