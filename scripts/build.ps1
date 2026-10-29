$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'environment.ps1')
foreach ($taskResource in @('src-tauri/resources/nemo-runtime/nemo_speech_asr_c.dll','src-tauri/resources/nemo-runtime/silero-v5.1.2.gguf','src-tauri/resources/live-runtime/moonshine.dll','src-tauri/resources/live-runtime/onnxruntime.dll','src-tauri/resources/live-models/vi/streaming_config.json','src-tauri/resources/live-models/en/streaming_config.json')) {
    if (!(Test-Path -LiteralPath $taskResource)) { throw 'Run python scripts/prepare-moonshine.py and python scripts/prepare-nemo.py before building. See MIGRATION.md.' }
}
npm.cmd run package -- -- --locked
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
