# Shared by build.ps1 and verify.ps1. Only changes the current process environment.
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $taskRoot

foreach ($taskBin in @(
    (Join-Path $env:USERPROFILE '.cargo\bin'),
    (Join-Path $env:ProgramFiles 'CMake\bin'),
    (Join-Path $env:USERPROFILE '.local\bin')
)) {
    if (Test-Path -LiteralPath $taskBin -PathType Container) {
        $env:PATH = "$taskBin;$env:PATH"
    }
}

if (!$env:LIBCLANG_PATH) {
    foreach ($taskClang in @(
        (Join-Path $taskRoot '.tools\libclang\clang\native'),
        (Join-Path $env:ProgramFiles 'LLVM\bin')
    )) {
        if (Test-Path -LiteralPath (Join-Path $taskClang 'libclang.dll') -PathType Leaf) {
            $env:LIBCLANG_PATH = $taskClang
            break
        }
    }
}
if (!$env:LIBCLANG_PATH -or !(Test-Path -LiteralPath (Join-Path $env:LIBCLANG_PATH 'libclang.dll') -PathType Leaf)) {
    throw 'Set LIBCLANG_PATH to a directory containing libclang.dll. See MIGRATION.md.'
}
if ($env:WHISPER_DONT_GENERATE_BINDINGS) {
    throw 'Remove WHISPER_DONT_GENERATE_BINDINGS: bindgen must generate Windows bindings.'
}
foreach ($taskCommand in @('node', 'npm.cmd', 'cargo', 'rustc', 'cmake')) {
    if (!(Get-Command $taskCommand -ErrorAction SilentlyContinue)) {
        throw "Missing $taskCommand. See MIGRATION.md for Windows build prerequisites."
    }
}
