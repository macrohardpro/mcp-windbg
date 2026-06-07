# Prefetch common Windows symbols to local cache
# Run this once to avoid slow symbol downloads during crash analysis
# Usage: .\scripts\prefetch_symbols.ps1

param(
    [string]$SymbolCache = "",
    [string]$SymbolServer = "https://msdl.microsoft.com/download/symbols",
    [string]$CdbPath = ""
)

$ErrorActionPreference = "Stop"

# -- Resolve symbol cache and server from _NT_SYMBOL_PATH if not specified ----
if ($env:_NT_SYMBOL_PATH) {
    # Parse first SRV* entry: SRV*cache*server
    if ($env:_NT_SYMBOL_PATH -match 'SRV\*([^*]+)\*([^*;]+)') {
        if (-not $SymbolCache) { $SymbolCache = $Matches[1] }
        if (-not $SymbolServer) { $SymbolServer = $Matches[2] }
    }
}
if (-not $SymbolCache) { $SymbolCache = "C:\Symbols" }
if (-not $SymbolServer) { $SymbolServer = "https://msdl.microsoft.com/download/symbols" }

# Find CDB
if (-not $CdbPath) {
    $cdbCandidates = @(
        "C:\Program Files (x86)\Windows Kits\10\Debuggers\x64\cdb.exe",
        "C:\Debuggers\x64\cdb.exe"
    )
    foreach ($c in $cdbCandidates) {
        if (Test-Path $c) {
            $CdbPath = $c
            break
        }
    }
}

if (-not $CdbPath -or -not (Test-Path $CdbPath)) {
    Write-Error "CDB not found. Specify -CdbPath or install Windows SDK Debugging Tools."
    exit 1
}

Write-Host "Using CDB: $CdbPath"
Write-Host "Symbol cache: $SymbolCache"
Write-Host "Symbol server: $SymbolServer"

# Respect existing _NT_SYMBOL_PATH if already set
if ($env:_NT_SYMBOL_PATH) {
    Write-Host "Using existing _NT_SYMBOL_PATH: $env:_NT_SYMBOL_PATH"
} else {
    $env:_NT_SYMBOL_PATH = "SRV*$SymbolCache*$SymbolServer"
    Write-Host "Set _NT_SYMBOL_PATH: $env:_NT_SYMBOL_PATH"
}

# Common system DLLs that crash dumps frequently reference
$systemDlls = @(
    "ntdll.dll",
    "kernel32.dll",
    "kernelbase.dll",
    "msvcrt.dll",
    "ucrtbase.dll",
    "vcruntime140.dll",
    "msvcp140.dll",
    "user32.dll",
    "gdi32.dll",
    "gdi32full.dll",
    "comctl32.dll",
    "combase.dll",
    "ole32.dll",
    "oleaut32.dll",
    "shell32.dll",
    "shlwapi.dll",
    "advapi32.dll",
    "ws2_32.dll",
    "winhttp.dll",
    "wininet.dll",
    "crypt32.dll",
    "bcrypt.dll",
    "bcryptprimitives.dll",
    "rpcrt4.dll",
    "sechost.dll",
    "imm32.dll",
    "setupapi.dll",
    "version.dll",
    "wldp.dll",
    "wow64.dll",
    "wow64cpu.dll",
    "wow64win.dll"
)

$systemRoot = [Environment]::GetEnvironmentVariable("SystemRoot")
$total = $systemDlls.Count
$done = 0
$failed = @()

Write-Host ""
Write-Host "Prefetching $total symbol sets..."

foreach ($dll in $systemDlls) {
    $done++
    $dllPath = Join-Path $systemRoot "System32\$dll"
    if (-not (Test-Path $dllPath)) {
        $dllPath = Join-Path $systemRoot "SysWOW64\$dll"
    }
    if (-not (Test-Path $dllPath)) {
        Write-Host "[$done/$total] SKIP $dll — file not found"
        continue
    }

    Write-Host "[$done/$total] $dll ... " -NoNewline

    $result = & $CdbPath -z $dllPath -c ".reload /f;q" 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "OK"
    } else {
        Write-Host "FAILED"
        $failed += $dll
    }
}

Write-Host ""
if ($failed.Count -eq 0) {
    Write-Host "All $total symbols prefetched successfully."
} else {
    Write-Host "Failed: $($failed.Count) / $total"
    Write-Host "Failed symbols: $($failed -join ', ')"
}

Write-Host "Symbol cache location: $SymbolCache"
Write-Host "Done."
