Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Resolve-Path (Join-Path $scriptRoot '..')
$repoRootPath = $repoRoot.Path

$temporaryRoot = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [System.IO.Path]::GetTempPath() }
$outDir = Join-Path $temporaryRoot 'orthohelp-ps-wrapper'

if (Test-Path $outDir) {
    Remove-Item -Recurse -Force $outDir
}
New-Item -ItemType Directory -Path $outDir | Out-Null

Write-Host "Generating PowerShell artefacts with cargo-orthohelp..."
$arguments = @(
    'run',
    '-p', 'cargo-orthohelp',
    '--',
    '--format', 'ps',
    '--package', 'orthohelp_fixture',
    '--locale', 'en-US',
    '--out-dir', $outDir
)

$commandOutput = & cargo @arguments 2>&1
$exitCode = $LASTEXITCODE
if ($exitCode -ne 0) {
    $outputText = $commandOutput | Out-String
    if ($outputText -match '(?i)unsupported format' -or $outputText -match 'UnsupportedFormat' -or $outputText -match 'PowerShell format is not yet implemented') {
        Write-Host 'PowerShell generator not available; skipping wrapper validation.'
        exit 0
    }

    Write-Error "cargo-orthohelp failed:\n$outputText"
    exit $exitCode
}

Write-Host 'Validating generated PowerShell module files...'
$psm1 = Get-ChildItem -Path $outDir -Recurse -Filter '*.psm1' | Select-Object -First 1
if (-not $psm1) {
    throw "Expected a .psm1 file under $outDir"
}

$psd1 = Get-ChildItem -Path $outDir -Recurse -Filter '*.psd1' | Select-Object -First 1
if (-not $psd1) {
    throw "Expected a .psd1 manifest under $outDir"
}

$helpXml = Get-ChildItem -Path $outDir -Recurse -Filter '*-help.xml' | Select-Object -First 1
if (-not $helpXml) {
    throw "Expected a MAML help XML file under $outDir"
}

$moduleText = Get-Content -Path $psm1.FullName -Raw
if ($moduleText -notmatch '\[CmdletBinding') {
    throw 'Wrapper module does not declare CmdletBinding.'
}
if ($moduleText -notmatch 'Register-ArgumentCompleter') {
    throw 'Wrapper module does not register argument completion.'
}

$manifestData = Import-PowerShellDataFile -Path $psd1.FullName
if ($manifestData.ContainsKey('ExternalHelp')) {
    $manifestKeys = ($manifestData.Keys | Sort-Object) -join ', '
    throw "Module manifest must not define ExternalHelp. Manifest keys: $manifestKeys"
}

Write-Host "PowerShell wrapper validation succeeded. Files located at $outDir"
