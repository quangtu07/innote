param(
  [string]$Destination = (Join-Path $PSScriptRoot '..\src-tauri\resources\diarization'),
  [switch]$Force
)
$ErrorActionPreference = 'Stop'
$manifest = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'diarization-resources.json') -Raw | ConvertFrom-Json
$Destination = [IO.Path]::GetFullPath($Destination)
$temp = Join-Path ([IO.Path]::GetTempPath()) ("innote-diarization-" + [guid]::NewGuid())
New-Item -ItemType Directory -Force -Path $temp | Out-Null
try {
  $sherpaRoot = Join-Path $temp 'sherpa'
  $segmentationRoot = Join-Path $temp 'segmentation'
  foreach ($archive in $manifest.archives) {
    $archivePath = Join-Path $temp $archive.file
    Write-Host "Download $($archive.url)"
    Invoke-WebRequest -UseBasicParsing $archive.url -OutFile $archivePath
    $target = if ($archive.extract -eq 'sherpa') { $sherpaRoot } else { $segmentationRoot }
    New-Item -ItemType Directory -Force -Path $target | Out-Null
    & tar -xjf $archivePath -C $target
    if ($LASTEXITCODE -ne 0) { throw "Cannot extract $($archive.file)" }
  }
  foreach ($file in $manifest.files) {
    Write-Host "Download $($file.url)"
    Invoke-WebRequest -UseBasicParsing $file.url -OutFile (Join-Path $temp $file.file)
  }
  $sherpaBin = Get-ChildItem -LiteralPath $sherpaRoot -Directory | Select-Object -First 1
  if (-not $sherpaBin) { throw 'Missing extracted sherpa directory' }
  New-Item -ItemType Directory -Force -Path $Destination | Out-Null
  Copy-Item -LiteralPath (Join-Path $sherpaBin.FullName 'bin\sherpa-onnx-offline-speaker-diarization.exe') -Destination $Destination -Force
  Copy-Item -LiteralPath (Join-Path $sherpaBin.FullName 'lib\sherpa-onnx-c-api.dll') -Destination $Destination -Force
  Copy-Item -LiteralPath (Join-Path $sherpaBin.FullName 'bin\onnxruntime.dll') -Destination $Destination -Force
  Copy-Item -LiteralPath (Join-Path $sherpaBin.FullName 'bin\onnxruntime_providers_shared.dll') -Destination $Destination -Force
  $segModel = Get-ChildItem -LiteralPath $segmentationRoot -Recurse -Filter 'model.int8.onnx' | Select-Object -First 1
  if (-not $segModel) { throw 'Missing model.int8.onnx' }
  Copy-Item -LiteralPath $segModel.FullName -Destination (Join-Path $Destination 'segmentation.int8.onnx') -Force
  Copy-Item -LiteralPath (Join-Path $temp 'campplus.onnx') -Destination (Join-Path $Destination 'campplus.onnx') -Force
  $license = Get-ChildItem -LiteralPath $segmentationRoot -Recurse -Filter 'LICENSE' | Select-Object -First 1
  if ($license) { Copy-Item -LiteralPath $license.FullName -Destination (Join-Path $Destination 'LICENSE-pyannote-segmentation.txt') -Force }
  try {
    Invoke-WebRequest -UseBasicParsing 'https://raw.githubusercontent.com/k2-fsa/sherpa-onnx/master/LICENSE' -OutFile (Join-Path $Destination 'LICENSE-sherpa-onnx.txt')
    Invoke-WebRequest -UseBasicParsing 'https://raw.githubusercontent.com/modelscope/3D-Speaker/main/LICENSE' -OutFile (Join-Path $Destination 'LICENSE-3dspeaker.txt')
  } catch {
    Write-Warning 'Could not download sherpa/3D-Speaker licenses; keeping existing files'
  }
  foreach ($expected in $manifest.outputs) {
    $path = Join-Path $Destination $expected.file
    if (-not (Test-Path -LiteralPath $path)) { throw "Missing $($expected.file)" }
    $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash
    if ($actual -ne $expected.sha256) { throw "Hash mismatch: $($expected.file)" }
  }
  Write-Host "Prepared diarization resources at $Destination" -ForegroundColor Green
} finally {
  Remove-Item -LiteralPath $temp -Recurse -Force -ErrorAction SilentlyContinue
}