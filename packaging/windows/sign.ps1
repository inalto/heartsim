<#
.SYNOPSIS
  Authenticode-signs the given files with signtool using the PFX in $env:WINDOWS_PFX_PATH.
.DESCRIPTION
  Used by .github/workflows/release.yml only when the WINDOWS_CERT_PFX_BASE64 secret exists. The timestamp keeps
  signatures valid after the certificate expires; override the server with $env:WINDOWS_TIMESTAMP_URL.
#>
param([Parameter(Mandatory = $true, ValueFromRemainingArguments = $true)][string[]]$Files)
$ErrorActionPreference = 'Stop'

$signtool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Recurse -Filter signtool.exe |
    Where-Object { $_.FullName -like '*\x64\*' } |
    Sort-Object FullName |
    Select-Object -Last 1
if (-not $signtool) { throw 'signtool.exe not found in the Windows SDK' }

$timestamp = if ($env:WINDOWS_TIMESTAMP_URL) { $env:WINDOWS_TIMESTAMP_URL } else { 'http://timestamp.digicert.com' }
& $signtool.FullName sign /fd sha256 /td sha256 /tr $timestamp /f $env:WINDOWS_PFX_PATH /p $env:WINDOWS_CERT_PASSWORD @Files
if ($LASTEXITCODE -ne 0) { throw "signtool sign failed ($LASTEXITCODE)" }
& $signtool.FullName verify /pa @Files
if ($LASTEXITCODE -ne 0) { throw "signtool verify failed ($LASTEXITCODE)" }
