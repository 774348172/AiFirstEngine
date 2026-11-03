#requires -Version 7.2
<#
.SYNOPSIS
Targeted Windows engine builds with a stable SSD cache and separate build/test evidence.
.EXAMPLE
./Invoke-EngineBuild.ps1 -Mode Test -Package engine_runtime -Features real-wgpu -Filter render_geometry::prepare_tasks::
.EXAMPLE
./Invoke-EngineBuild.ps1 -Mode Test -Package engine_runtime -Features real-wgpu -Lib -TestTarget platform_3d_visual,unified_render_execution_u2
.EXAMPLE
./Invoke-EngineBuild.ps1 -Mode Release -Package engine_runtime_host
#>
[CmdletBinding()]
param(
    [ValidateSet('Test', 'Check', 'Clippy', 'Release')][string]$Mode = 'Test',
    [Parameter(Mandatory)][ValidatePattern('^[a-zA-Z0-9_-]+$')][string]$Package,
    [string]$Features = '',
    [switch]$NoDefaultFeatures,
    [switch]$Lib,
    [string[]]$TestTarget = @(),
    [string]$Filter = '',
    [switch]$IncludeIgnored,
    [ValidateSet('dev-fast', 'dev', 'test')][string]$Profile = 'dev-fast',
    [string]$TargetDir = 'I:/gameEngin-build/371/cargo-target',
    [string]$EvidenceDir,
    [ValidateRange(30, 7200)][int]$TimeoutSeconds = 1200,
    [switch]$PlanOnly
)
$ErrorActionPreference = 'Stop'
$runWatch = [Diagnostics.Stopwatch]::StartNew()
$repo = Split-Path $PSScriptRoot -Parent
$manifest = Join-Path $PSScriptRoot 'Cargo.toml'
if ($Mode -ne 'Test' -and ($Filter -or $IncludeIgnored -or $TestTarget.Count)) {
    throw 'Test selectors require -Mode Test.'
}
if (-not [IO.Path]::IsPathFullyQualified($TargetDir)) { throw 'TargetDir must be absolute.' }
$TargetDir = [IO.Path]::GetFullPath($TargetDir)
if ($Mode -eq 'Release') { $selectedProfile = 'release' } else { $selectedProfile = $Profile }
$verb = switch ($Mode) { Test { 'test' } Check { 'check' } Clippy { 'clippy' } Release { 'build' } }
$buildArgs = @($verb, '--manifest-path', $manifest, '--locked', '-j1', '--target-dir', $TargetDir,
    '-p', $Package, '--profile', $selectedProfile, '--timings', '--message-format=json-render-diagnostics')
if ($Features) { $buildArgs += @('--features', $Features) }
if ($NoDefaultFeatures) { $buildArgs += '--no-default-features' }
if ($Lib -or ($Mode -eq 'Test' -and -not $TestTarget.Count)) { $buildArgs += '--lib' }
foreach ($target in $TestTarget) { $buildArgs += @('--test', $target) }
if ($Mode -eq 'Test') { $buildArgs += '--no-run' }
$plan = [ordered]@{ mode=$Mode; package=$Package; profile=$selectedProfile; targetDir=$TargetDir;
    buildWorkingDirectory=$PSScriptRoot; testWorkingDirectory=$repo;
    executable='cargo'; arguments=$buildArgs; filter=$Filter; includeIgnored=[bool]$IncludeIgnored;
    formalQualified=$false }
if ($PlanOnly) { $plan | ConvertTo-Json -Depth 6; return }
if (-not (Test-Path -LiteralPath ([IO.Path]::GetPathRoot($TargetDir)))) {
    throw 'Configured target volume is unavailable; select an explicit target, no automatic fallback.'
}
if (-not $EvidenceDir) {
    $EvidenceDir = Join-Path $repo ('.artifacts/371/build-feedback/{0}-{1}' -f (Get-Date -Format 'yyyyMMdd-HHmmss-fff'), $Mode)
}
$EvidenceDir = [IO.Path]::GetFullPath($EvidenceDir)
if (Test-Path -LiteralPath $EvidenceDir) { throw 'EvidenceDir must be new; previous results are never overwritten.' }
New-Item -ItemType Directory -Path $EvidenceDir -Force | Out-Null
New-Item -ItemType Directory -Path $TargetDir -Force | Out-Null
$plan | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $EvidenceDir 'plan.json') -Encoding utf8
$receipt = [ordered]@{ startedUtc=[DateTime]::UtcNow.ToString('o'); plan=$plan;
    runnerSha256=(Get-FileHash -LiteralPath $PSCommandPath).Hash;
    manifestSha256=(Get-FileHash -LiteralPath $manifest).Hash; lockSha256=(Get-FileHash -LiteralPath (Join-Path $PSScriptRoot 'Cargo.lock')).Hash;
    environment=@{}; build=$null; tests=@(); status='running'; formalQualified=$false }
Get-ChildItem Env: | Where-Object Name -Match '^(RUSTFLAGS|CARGO_ENCODED_RUSTFLAGS|RUSTC_WRAPPER|RUSTC_WORKSPACE_WRAPPER|CARGO_PROFILE_.+|CARGO_INCREMENTAL|CARGO_BUILD_JOBS)$' |
    ForEach-Object { $receipt.environment[$_.Name] = $_.Value }
function Save-Receipt { $receipt | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $EvidenceDir 'result.json') -Encoding utf8 }
function Invoke-LoggedProcess([string]$Executable, [string[]]$Arguments, [string]$Name, [string]$WorkingDirectory = $repo) {
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $Executable
    $info.WorkingDirectory = $WorkingDirectory
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    foreach ($argument in $Arguments) { $info.ArgumentList.Add($argument) }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $info
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $timedOut = $false
    $started = $false
    try {
        if (-not $process.Start()) { throw "Could not start $Executable" }
        $started = $true
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        while (-not $process.WaitForExit(500)) {
            if ($watch.Elapsed.TotalSeconds -ge $TimeoutSeconds) {
                $timedOut = $true
                $process.Kill($true)
                $process.WaitForExit()
                break
            }
        }
        $watch.Stop()
        $stdout.GetAwaiter().GetResult() | Set-Content -LiteralPath (Join-Path $EvidenceDir "$Name.stdout.log") -Encoding utf8
        $stderr.GetAwaiter().GetResult() | Set-Content -LiteralPath (Join-Path $EvidenceDir "$Name.stderr.log") -Encoding utf8
        return [ordered]@{ exitCode=$process.ExitCode; wallSeconds=$watch.Elapsed.TotalSeconds; timedOut=$timedOut }
    } finally {
        if ($started -and -not $process.HasExited) { $process.Kill($true); $process.WaitForExit() }
        $process.Dispose()
    }
}
try {
    $receipt.build = Invoke-LoggedProcess 'cargo' $buildArgs 'build' $PSScriptRoot
    $receipt.build.timingReport = Join-Path $TargetDir 'cargo-timings/cargo-timing.html'
    if (Test-Path -LiteralPath $receipt.build.timingReport) {
        Copy-Item -LiteralPath $receipt.build.timingReport -Destination (Join-Path $EvidenceDir 'cargo-timing.html')
    }
    Save-Receipt
    if ($receipt.build.exitCode -ne 0 -or $receipt.build.timedOut) { throw 'Build failed; inspect build.stderr.log and build.stdout.log.' }
    if ($Mode -eq 'Test') {
        $binaries = @(Get-Content -LiteralPath (Join-Path $EvidenceDir 'build.stdout.log') | ForEach-Object {
            $item = $_ | ConvertFrom-Json
            if ($item.reason -eq 'compiler-artifact' -and $item.profile.test -and $item.executable) { $item }
        })
        if (-not $binaries.Count) { throw 'Cargo returned no runnable test artifacts.' }
        foreach ($binary in $binaries) {
            $testArgs = @('--nocapture', '--test-threads=1')
            if ($IncludeIgnored) { $testArgs += '--include-ignored' }
            if ($Filter) { $testArgs = @($Filter) + $testArgs }
            $result = Invoke-LoggedProcess $binary.executable $testArgs ("test-{0}" -f $receipt.tests.Count)
            $result.executable = $binary.executable
            $result.sha256 = (Get-FileHash -LiteralPath $binary.executable).Hash
            $result.target = $binary.target.name
            $result.fresh = $binary.fresh
            $result.compilationProfile = $binary.profile
            $testOutput = Get-Content -LiteralPath (Join-Path $EvidenceDir ("test-{0}.stdout.log" -f $receipt.tests.Count)) -Raw
            $summaries = [regex]::Matches($testOutput, '(?m)^test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed;')
            $summary = if ($summaries.Count) { $summaries[$summaries.Count - 1] } else { [regex]::Match('', 'never') }
            $result.passed = if ($summary.Success) { [int]$summary.Groups[1].Value } else { 0 }
            $result.failed = if ($summary.Success) { [int]$summary.Groups[2].Value } else { 0 }
            $receipt.tests += $result
            Save-Receipt
            if ($result.exitCode -ne 0 -or $result.timedOut) { throw "Test failed: $($binary.target.name)" }
            if (-not $summary.Success -or ($result.passed + $result.failed) -eq 0) {
                throw "No tests executed: $($binary.target.name); verify the filter and composition."
            }
        }
    }
    $receipt.status = 'passed'
} catch {
    $receipt.status = 'failed'
    $receipt.error = $_.ToString()
    throw
} finally {
    $receipt.finishedUtc = [DateTime]::UtcNow.ToString('o')
    $receipt.elapsedWallSeconds = $runWatch.Elapsed.TotalSeconds
    # Ordered dictionaries are not property-expanded by Measure-Object.
    [double]$testSeconds = 0
    foreach ($testResult in $receipt.tests) { $testSeconds += [double]$testResult.wallSeconds }
    $receipt.overheadSeconds = $receipt.elapsedWallSeconds - $receipt.build.wallSeconds - $testSeconds
    Save-Receipt
    Write-Host "Build evidence: $EvidenceDir"
}
