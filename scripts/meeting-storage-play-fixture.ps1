param([Parameter(Mandatory=$true)][string]$AudioPath)
$ErrorActionPreference='Stop'
$taskAudio=(Resolve-Path -LiteralPath $AudioPath).Path
$taskPlayer=New-Object System.Media.SoundPlayer $taskAudio
$taskPlayer.PlaySync()
$taskPlayer.Dispose()
