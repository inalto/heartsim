<#
.SYNOPSIS
  Asserts that heartsim.exe is a GUI-subsystem binary (no console window) with a statically linked MSVC runtime.
#>
param([Parameter(Mandatory = $true)][string]$Exe)
$ErrorActionPreference = 'Stop'

$bytes = [IO.File]::ReadAllBytes($Exe)
$pe = [BitConverter]::ToInt32($bytes, 0x3C)
# IMAGE_OPTIONAL_HEADER starts 24 bytes after the PE signature; Subsystem sits at offset 68 in PE32 and PE32+.
$subsystem = [BitConverter]::ToUInt16($bytes, $pe + 24 + 68)
if ($subsystem -ne 2) { throw "expected IMAGE_SUBSYSTEM_WINDOWS_GUI (2), got ${subsystem}: a console window would appear" }

$text = [Text.Encoding]::ASCII.GetString($bytes)
foreach ($dll in 'VCRUNTIME140', 'MSVCP140') {
    if ($text -match $dll) { throw "$dll.dll is imported: the MSVC runtime must be linked statically (+crt-static)" }
}
Write-Host "${Exe}: GUI subsystem, static CRT"
