$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Speech
$taskDir = Join-Path (Get-Location) 'test-results\fixtures\realtime'
New-Item -ItemType Directory -Force -Path $taskDir | Out-Null
$taskSynth = New-Object System.Speech.Synthesis.SpeechSynthesizer
try {
    $taskVoice = $taskSynth.GetInstalledVoices() | Where-Object { $_.Enabled -and $_.VoiceInfo.Culture.Name -eq 'en-US' } | Select-Object -First 1
    if (!$taskVoice) { throw 'No installed English voice for the public reference fixture' }
    $taskSynth.SelectVoice($taskVoice.VoiceInfo.Name)
    $taskLines = @(
        'The engineering team will finish the prototype on Friday.',
        'Ms. Nguyen approved fifteen million dong, not fifty million.',
        'Please do not send the invoice before the meeting.'
    )
    for ($taskIndex = 0; $taskIndex -lt $taskLines.Count; $taskIndex++) {
        $taskSynth.SetOutputToWaveFile((Join-Path $taskDir "part-$taskIndex.wav"))
        $taskSynth.Speak($taskLines[$taskIndex])
        $taskSynth.SetOutputToNull()
    }
    $taskLines | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskDir 'reference.json') -Encoding utf8
    Write-Output 'Generated three public English reference utterances locally; no audio/API upload.'
} finally { $taskSynth.Dispose() }
