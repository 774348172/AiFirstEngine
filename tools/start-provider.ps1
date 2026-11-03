param([Parameter(Mandatory=$true)][string]$WorkspaceRoot,[string]$ProjectRoot)
$ErrorActionPreference='Stop'
$releaseRoot=Split-Path -Parent $PSScriptRoot
$providerPath=Join-Path $releaseRoot '.build/target/release/ai_engine_tool_provider_mcp.exe'
if(!(Test-Path -LiteralPath $providerPath)){throw 'Run tools/build-provider.ps1 first'}
$providerArgs=@('--workspace-root',$WorkspaceRoot)
if($ProjectRoot){$providerArgs+=@('--project-root',$ProjectRoot)}
& $providerPath @providerArgs
exit $LASTEXITCODE
