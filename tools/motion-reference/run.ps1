param([Parameter(ValueFromRemainingArguments = $true)][string[]]$ToolArgs)
$ErrorActionPreference = 'Stop'
$linuxPath = (& wsl.exe wslpath -u $PSScriptRoot).Trim()
if ($LASTEXITCODE -ne 0) { throw 'WSL is required for the local processing environment.' }
& wsl.exe bash "$linuxPath/run.sh" @ToolArgs
exit $LASTEXITCODE
