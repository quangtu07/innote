param(
  [Parameter(Mandatory = $true)] [string]$Repo,
  [Parameter(Mandatory = $true)] [string]$Version,
  [string]$Notes = "",
  [string]$KeyPath = "$env:USERPROFILE\.tauri\innote.key",
  [string]$PasswordPath = "$env:USERPROFILE\.tauri\innote.key.password"
)

$ErrorActionPreference = "Stop"
$project = Split-Path $PSScriptRoot -Parent
$repo = $Repo.Trim().Trim("/")
if ($repo -notmatch "^[^/]+/[^/]+$") {
  throw "Repo phải có dạng OWNER/innote, ví dụ: alexlinh/innote"
}
if (!(Test-Path -LiteralPath $KeyPath)) { throw "Không thấy private key: $KeyPath" }
if (!(Test-Path -LiteralPath $PasswordPath)) { throw "Không thấy password: $PasswordPath" }

$configPath = Join-Path $project "src-tauri\tauri.conf.json"
$packagePath = Join-Path $project "package.json"
$cargoPath = Join-Path $project "src-tauri\Cargo.toml"
$config = [IO.File]::ReadAllText($configPath, [Text.Encoding]::UTF8) | ConvertFrom-Json
$package = Get-Content -LiteralPath $packagePath -Raw | ConvertFrom-Json
$cargo = [IO.File]::ReadAllText($cargoPath, [Text.Encoding]::UTF8)

if ($config.version -ne $Version -or $package.version -ne $Version) {
  throw "Version chưa đồng bộ. Cần cùng $Version trong package.json, package-lock.json, Cargo.toml, Cargo.lock và tauri.conf.json."
}
$cargoVersionPattern = 'version\s*=\s*"' + [regex]::Escape($Version) + '"'
if ($cargo -notmatch $cargoVersionPattern) {
  throw "Version chưa đồng bộ trong Cargo.toml. Cần cùng $Version."
}

$endpoint = "https://github.com/$repo/releases/latest/download/latest.json"
$config.plugins.updater.endpoints = @($endpoint)
$configJson = $config | ConvertTo-Json -Depth 20
[IO.File]::WriteAllText($configPath, $configJson + "`n", (New-Object Text.UTF8Encoding($false)))

$env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content -LiteralPath $KeyPath -Raw)
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ((Get-Content -LiteralPath $PasswordPath -Raw).Trim())

function Invoke-NpmBuild([string[]]$Arguments) {
  $old = $ErrorActionPreference
  $ErrorActionPreference = "Continue"
  try {
    & npm.cmd @Arguments 2>&1 | ForEach-Object {
      if ($_ -is [System.Management.Automation.ErrorRecord]) {
        Write-Host $_.Exception.Message
      } else {
        Write-Host $_
      }
    }
    $code = $LASTEXITCODE
  } finally {
    $ErrorActionPreference = $old
  }
  if ($code -ne 0) { throw "npm.cmd $($Arguments -join ' ') thất bại (exit $code)." }
}

Push-Location $project
try {
  . .\scripts\environment.ps1
  Write-Host "Đang build Innote $Version..." -ForegroundColor Cyan
  Invoke-NpmBuild @("run", "package", "--", "--no-bundle", "--", "--locked")
  Invoke-NpmBuild @("run", "tauri", "--", "bundle", "--bundles", "nsis", "--ci", "--no-binary-patching")

  $bundle = Join-Path $project "src-tauri\target\release\bundle\nsis"
  $rawSetup = Get-ChildItem -LiteralPath $bundle -Filter "Innote_${Version}_x64-setup.exe" | Select-Object -First 1
  if (!$rawSetup) { throw "Không thấy installer trong $bundle" }
  $rawSig = "$($rawSetup.FullName).sig"
  if (!(Test-Path -LiteralPath $rawSig)) { throw "Không thấy file .sig. Kiểm tra khóa ký." }

  $releaseDir = Join-Path $project "release"
  New-Item -ItemType Directory -Force -Path $releaseDir | Out-Null
  $asset = Join-Path $releaseDir "Innote-Setup-$Version.exe"
  $sig = "$asset.sig"
  Copy-Item -LiteralPath $rawSetup.FullName -Destination $asset -Force
  Copy-Item -LiteralPath $rawSig -Destination $sig -Force

  $manifest = [ordered]@{
    version = $Version
    notes = $Notes
    pub_date = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    platforms = [ordered]@{
      "windows-x86_64" = [ordered]@{
        signature = (Get-Content -LiteralPath $sig -Raw).Trim()
        url = "https://github.com/$repo/releases/download/v$Version/Innote-Setup-$Version.exe"
      }
    }
  }
  $latest = Join-Path $releaseDir "latest.json"
  $latestJson = $manifest | ConvertTo-Json -Depth 6
  [IO.File]::WriteAllText($latest, $latestJson + "`n", (New-Object Text.UTF8Encoding($false)))

  Write-Host ""
  Write-Host "Xong. Upload 3 file này vào GitHub Release v${Version}:" -ForegroundColor Green
  Write-Host "  $asset"
  Write-Host "  $sig"
  Write-Host "  $latest"
  Write-Host ""
  Write-Host "Endpoint: $endpoint"
} finally {
  Pop-Location
}