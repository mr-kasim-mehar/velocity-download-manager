$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$root = Split-Path -Parent $PSScriptRoot
$tools = Join-Path $root "src-tauri\tools"
$temp = Join-Path ([System.IO.Path]::GetTempPath()) ("velocity-tools-" + [guid]::NewGuid())
New-Item -ItemType Directory -Force -Path $tools, $temp | Out-Null

function Get-VerifiedFile {
  param([string]$Url, [string]$Destination, [string]$Sha256)
  & curl.exe -fL --retry 4 --retry-delay 2 --output $Destination $Url
  if ($LASTEXITCODE -ne 0) { throw "Download failed: $Url" }
  $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $Destination).Hash
  if ($actual -ne $Sha256) { throw "SHA-256 mismatch for $Destination. Expected $Sha256, got $actual" }
}

try {
  Get-VerifiedFile `
    "https://github.com/yt-dlp/yt-dlp/releases/download/2026.08.19/yt-dlp.exe" `
    (Join-Path $tools "yt-dlp.exe") `
    "66674953FE251B89F4D08C5F0E35E0728679BD67AB3D7D05C0562AF101DD3E7A"

  $denoArchive = Join-Path $temp "deno.zip"
  Get-VerifiedFile `
    "https://github.com/denoland/deno/releases/download/v2.9.7/deno-x86_64-pc-windows-msvc.zip" `
    $denoArchive `
    "A0C3101B4158D1DFB7D6A78A7BF0F3DE80C96BB423C152BEEC8BEB22786F2238"
  Expand-Archive -LiteralPath $denoArchive -DestinationPath (Join-Path $temp "deno") -Force
  Copy-Item -LiteralPath (Join-Path $temp "deno\deno.exe") -Destination $tools -Force

  $ariaArchive = Join-Path $temp "aria2.zip"
  Get-VerifiedFile `
    "https://github.com/aria2/aria2/releases/download/release-1.37.0/aria2-1.37.0-win-64bit-build1.zip" `
    $ariaArchive `
    "67D015301EEF0B612191212D564C5BB0A14B5B9C4796B76454276A4D28D9B288"
  Expand-Archive -LiteralPath $ariaArchive -DestinationPath (Join-Path $temp "aria2") -Force
  $aria = Get-ChildItem (Join-Path $temp "aria2") -Filter aria2c.exe -Recurse | Select-Object -First 1
  if (-not $aria) { throw "aria2c.exe was not found in the downloaded archive" }
  Copy-Item -LiteralPath $aria.FullName -Destination $tools -Force

  $ffmpegArchive = Join-Path $temp "ffmpeg.zip"
  Get-VerifiedFile `
    "https://github.com/yt-dlp/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip" `
    $ffmpegArchive `
    "75104CEAFCB51A7982B248DF5113D3FBB449D1C32425F1EDF184D5CE391C3B9B"
  Expand-Archive -LiteralPath $ffmpegArchive -DestinationPath (Join-Path $temp "ffmpeg") -Force
  foreach ($name in @("ffmpeg.exe", "ffprobe.exe")) {
    $binary = Get-ChildItem (Join-Path $temp "ffmpeg") -Filter $name -Recurse | Select-Object -First 1
    if (-not $binary) { throw "$name was not found in the downloaded archive" }
    Copy-Item -LiteralPath $binary.FullName -Destination $tools -Force
  }

  $expectedBinaries = @{
    "aria2c.exe"  = "BE2099C214F63A3CB4954B09A0BECD6E2E34660B886D4C898D260FEBFE9D70C2"
    "deno.exe"    = "E020F3E232BD16E33768DEE528E5983349C962952051CED0A5D58AD42F5D9B33"
    "ffmpeg.exe"  = "24B6A221EDDD1109D1A24FBEE86FFEAD0C9F989832EB118BD09629C9F924585D"
    "ffprobe.exe" = "17D9DCD3DDC062FC631DB68253F24D999B942C5CECA44B269F25529678D42A5B"
  }
  foreach ($entry in $expectedBinaries.GetEnumerator()) {
    $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $tools $entry.Key)).Hash
    if ($actual -ne $entry.Value) { throw "SHA-256 mismatch for $($entry.Key)" }
  }
  Write-Host "Windows helper tools are ready and verified."
}
finally {
  Remove-Item -LiteralPath $temp -Recurse -Force -ErrorAction SilentlyContinue
}
