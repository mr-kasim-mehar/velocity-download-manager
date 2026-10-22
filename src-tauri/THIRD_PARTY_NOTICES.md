# Third-party components

Velocity bundles or interoperates with the following command-line workers:

## yt-dlp

- Project: https://github.com/yt-dlp/yt-dlp
- Primary license: The Unlicense
- Standalone releases contain additional third-party components under their respective licenses. See the `THIRD_PARTY_LICENSES.txt` distributed by yt-dlp for the exact build.

## aria2

- Project: https://github.com/aria2/aria2
- License: GNU General Public License version 2 or later
- Velocity invokes aria2 as a separate command-line program and does not link to its program code.

The copyright and license notices shipped with the corresponding binaries remain authoritative.

## Deno

- Project: https://github.com/denoland/deno
- License: MIT
- Deno is launched on demand as the JavaScript runtime used by yt-dlp extractors.

## FFmpeg

- Project: https://ffmpeg.org/
- Bundled build: https://github.com/yt-dlp/FFmpeg-Builds
- License: GNU General Public License for the bundled GPL build
- FFmpeg and FFprobe are launched on demand to merge separate media streams.
