#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TOOLS="$ROOT/src-tauri/tools"
TEMP="$(mktemp -d)"
trap 'rm -rf "$TEMP"' EXIT
mkdir -p "$TOOLS"

curl -fL --retry 4 --retry-delay 2 \
  https://github.com/yt-dlp/yt-dlp/releases/download/2026.08.19/yt-dlp \
  -o "$TOOLS/yt-dlp"
curl -fL --retry 4 --retry-delay 2 \
  https://github.com/denoland/deno/releases/download/v2.9.7/deno-x86_64-unknown-linux-gnu.zip \
  -o "$TEMP/deno.zip"
unzip -q "$TEMP/deno.zip" -d "$TEMP/deno"
cp "$TEMP/deno/deno" "$TOOLS/deno"

curl -fL --retry 4 --retry-delay 2 \
  https://github.com/yt-dlp/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linux64-gpl.tar.xz \
  -o "$TEMP/ffmpeg.tar.xz"
mkdir -p "$TEMP/ffmpeg"
tar -xf "$TEMP/ffmpeg.tar.xz" -C "$TEMP/ffmpeg"
find "$TEMP/ffmpeg" -type f -name ffmpeg -exec cp '{}' "$TOOLS/ffmpeg" \;
find "$TEMP/ffmpeg" -type f -name ffprobe -exec cp '{}' "$TOOLS/ffprobe" \;

# aria2 is supplied by Ubuntu's signed package repository for the Linux build.
cp /usr/bin/aria2c "$TOOLS/aria2c"
chmod +x "$TOOLS/yt-dlp" "$TOOLS/deno" "$TOOLS/ffmpeg" "$TOOLS/ffprobe" "$TOOLS/aria2c"

for tool in yt-dlp deno ffmpeg ffprobe aria2c; do
  test -x "$TOOLS/$tool" || { echo "Missing helper tool: $tool" >&2; exit 1; }
done

echo "Linux helper tools are ready."
