param([switch]$Spine)
$ErrorActionPreference='Stop'
$releaseRoot=Split-Path -Parent $PSScriptRoot
$savedTarget=$env:CARGO_TARGET_DIR
try {
 $env:CARGO_TARGET_DIR=Join-Path $releaseRoot '.build/target'
 Push-Location (Join-Path $releaseRoot 'rust')
 try {
  $buildArgs=@('build','--release','--locked','-p','engine_tool_provider','-p','engine_runtime_host','-p','runtime_cli','--lib','--bin','ai_engine_tool_provider_mcp','--bin','ai_project_runtime_player','--bin','ai_engine_runtime_cli','--features')
  $features='runtime_cli/real-window'
  if($Spine){$features+=',engine_tool_provider/spine,engine_runtime_host/spine,runtime_cli/spine'}
  $buildArgs+=$features
  & cargo @buildArgs
  if($LASTEXITCODE -ne 0){throw 'V0.0.7 Release build failed'}
 } finally {Pop-Location}
} finally {$env:CARGO_TARGET_DIR=$savedTarget}
