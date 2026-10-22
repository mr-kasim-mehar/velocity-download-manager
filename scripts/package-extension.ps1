$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$source = Join-Path $root "browser-extension\*"
$destination = Join-Path $root "src-tauri\browser-extension.zip"
Remove-Item -LiteralPath $destination -Force -ErrorAction SilentlyContinue
Compress-Archive -Path $source -DestinationPath $destination -CompressionLevel Optimal
Write-Host "Created $destination"
