param([switch]$WebViewDebug)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'environment.ps1')

# Do not replace a running recorder. Closing it normally also stops captions.
if (Get-Process innote -ErrorAction SilentlyContinue) {
    throw 'Đóng cửa sổ Innote hiện tại trước khi chạy development để tránh dùng backend cũ. Không dừng một cuộc họp đang ghi.'
}
if ($WebViewDebug) {
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9223'
}

$captionListener = Get-NetTCPConnection -LocalPort 1420 -State Listen -ErrorAction SilentlyContinue
if ($captionListener) {
    $captionVite = Get-CimInstance Win32_Process | Where-Object {
        $_.Name -eq 'node.exe' -and $_.CommandLine -and
        $_.CommandLine.Contains($taskRoot) -and $_.CommandLine.Contains('vite')
    }
    if (!$captionVite) { throw 'Cổng 1420 đang do tiến trình khác dùng. Không kết nối frontend của repo khác.' }
    $captionConfig = Join-Path ([System.IO.Path]::GetTempPath()) ('innote-dev-' + [guid]::NewGuid() + '.json')
    [System.IO.File]::WriteAllText($captionConfig, '{"build":{"beforeDevCommand":""}}')
    try {
        npm.cmd run tauri -- dev --no-watch --config $captionConfig
        $captionExit = $LASTEXITCODE
    } finally {
        Remove-Item -LiteralPath $captionConfig -ErrorAction SilentlyContinue
    }
} else {
    npm.cmd run tauri -- dev --no-watch
    $captionExit = $LASTEXITCODE
}
exit $captionExit
