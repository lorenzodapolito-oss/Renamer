$ErrorActionPreference = 'Stop'
$root = Resolve-Path "$PSScriptRoot\..\.."
$dist = Join-Path $root 'dist\Renamer-portable'
New-Item -ItemType Directory -Force $dist | Out-Null
Copy-Item (Join-Path $root 'target\release\renamer.exe') $dist -Force
Copy-Item (Join-Path $root 'README.md') $dist -Force
Write-Host "Pronto: $dist\renamer.exe"
