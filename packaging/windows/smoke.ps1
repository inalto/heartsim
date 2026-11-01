<#
.SYNOPSIS
  Installs an MSI or NSIS installer silently, runs `heartsim --version`, and uninstalls again.
.PARAMETER Installer
  Path to the .msi or *-setup.exe.
.PARAMETER Version
  Expected version; the binary must print `heartsim <Version>`.
#>
param(
    [Parameter(Mandatory = $true)][string]$Installer,
    [Parameter(Mandatory = $true)][string]$Version
)
$ErrorActionPreference = 'Stop'
$Installer = (Resolve-Path $Installer).Path
$isMsi = $Installer.EndsWith('.msi')

function Find-Exe([string[]]$Roots) {
    foreach ($root in $Roots) {
        $hit = Get-ChildItem $root -Filter heartsim.exe -Recurse -Depth 3 -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($hit) { return $hit.FullName }
    }
    throw "heartsim.exe not found under: $($Roots -join ', ')"
}

if ($isMsi) {
    $p = Start-Process msiexec.exe -ArgumentList '/i', "`"$Installer`"", '/qn', '/norestart', '/l*v', 'msi-install.log' -Wait -PassThru
    if ($p.ExitCode -ne 0) { throw "msiexec install failed ($($p.ExitCode)); see msi-install.log" }
    $exe = Find-Exe @($env:ProgramFiles)
} else {
    $p = Start-Process $Installer -ArgumentList '/S' -Wait -PassThru
    if ($p.ExitCode -ne 0) { throw "NSIS install failed ($($p.ExitCode))" }
    $exe = Find-Exe @($env:LOCALAPPDATA)
}
Write-Host "installed: $exe"

# The release binary uses the GUI subsystem, so stdout must be redirected to be captured.
$out = Join-Path $env:RUNNER_TEMP 'version.txt'
$p = Start-Process $exe -ArgumentList '--version' -NoNewWindow -Wait -PassThru -RedirectStandardOutput $out
if ($p.ExitCode -ne 0) { throw "heartsim --version exited with $($p.ExitCode)" }
$printed = (Get-Content $out -Raw).Trim()
if ($printed -ne "heartsim $Version") { throw "unexpected --version output: '$printed'" }
Write-Host $printed

if ($isMsi) {
    $p = Start-Process msiexec.exe -ArgumentList '/x', "`"$Installer`"", '/qn', '/norestart' -Wait -PassThru
    if ($p.ExitCode -ne 0) { throw "msiexec uninstall failed ($($p.ExitCode))" }
} else {
    $uninstaller = Join-Path (Split-Path $exe) 'uninstall.exe'
    $p = Start-Process $uninstaller -ArgumentList '/S' -Wait -PassThru
    if ($p.ExitCode -ne 0) { throw "NSIS uninstall failed ($($p.ExitCode))" }
}
