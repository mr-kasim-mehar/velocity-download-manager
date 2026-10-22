#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TOOLS="$ROOT/src-tauri/tools"
TEMP="$(mktemp -d)"
trap 'rm -rf "$TEMP"' EXIT
mkdir -p "$TOOLS"

download_verified() {
  local url="$1"
  local destination="$2"
  local sha256="$3"
  curl -fL --retry 4 --retry-delay 2 "$url" -o "$destination"
  echo "$sha256  $destination" | sha256sum --check --status || {
    echo "SHA-256 mismatch for $destination" >&2
    exit 1
  }
}

download_verified \
  "https://github.com/yt-dlp/yt-dlp/releases/download/2026.08.19/yt-dlp" \
  "$TOOLS/yt-dlp" \
  "1fa6733c37ea6fb51c99ad8fe785e7b7e5f3246c9b980230329d4fb72ed8d4d6"
download_verified \
  "https://github.com/denoland/deno/releases/download/v2.9.7/deno-x86_64-unknown-linux-gnu.zip" \
  "$TEMP/deno.zip" \
  "c6527f24f4b16031d3ae4fa9f658d5f11534c8d84ce7dc8502420280919c3490"
unzip -q "$TEMP/deno.zip" -d "$TEMP/deno"
cp "$TEMP/deno/deno" "$TOOLS/deno"

download_verified \
  "https://github.com/yt-dlp/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linux64-gpl.tar.xz" \
  "$TEMP/ffmpeg.tar.xz" \
  "ffa15db386dca7f89cfa8574849f8d3e77c80ba3d17f57c4eb01d9e133272988"
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
