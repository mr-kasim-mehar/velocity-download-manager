# Velocity Download Manager

[![Continuous integration](https://github.com/mr-kasim-mehar/velocity-download-manager/actions/workflows/ci.yml/badge.svg)](https://github.com/mr-kasim-mehar/velocity-download-manager/actions/workflows/ci.yml)
[![MIT License](https://img.shields.io/badge/license-MIT-22c55e.svg)](LICENSE)
[![Latest release](https://img.shields.io/github/v/release/mr-kasim-mehar/velocity-download-manager)](https://github.com/mr-kasim-mehar/velocity-download-manager/releases)

Velocity is a native cross-platform download manager for Windows and Linux. It supports resumable HTTP downloads, supported media pages through yt-dlp, magnet links and `.torrent` files through aria2, persistent history, pause/resume controls, transfer speed and ETA, configurable download locations, native file import, and a professional desktop UI.

Velocity is free and open-source software developed by **Muhammad Qasim**,
security researcher, application developer, and ethical hacker. The complete
functional and non-functional specification is in
[`docs/PRODUCT_REQUIREMENTS.md`](docs/PRODUCT_REQUIREMENTS.md).

Windows release installers bundle yt-dlp, Deno, FFmpeg, and aria2 so media and torrent downloads work without separate user setup. Media jobs support Best, 4K, 2K, 1080p, 720p, 480p, and 360p video presets plus MP3, M4A, AAC, Opus, FLAC, and WAV audio-only output. FFmpeg merges or converts streams locally. Auto detection recognizes common media services, magnet links, torrent files, obvious direct-file URLs, and treats extensionless web pages as media candidates. The Add Download dialog also lets users explicitly choose the source type when a URL is ambiguous.

Use Velocity only for content you own or are authorized to download. It does not bypass DRM or access controls.

## Development

Requirements: Node.js 20+, Rust 1.90+, and the [Tauri system prerequisites](https://v2.tauri.app/start/prerequisites/) for your operating system.

```bash
npm install
npm run desktop:dev
```

## Create installers

Build on each target operating system because desktop bundles are platform-native:

```bash
npm install
npm run desktop:build
```

- Windows outputs an NSIS `.exe` and WiX `.msi` under `src-tauri/target/release/bundle/`.
- Ubuntu/Debian outputs `.deb` and AppImage bundles under the same bundle directory.
- Other Linux distributions can use the AppImage. RPM packaging can be added to the Tauri target list for Fedora-family systems.

The repository does not store the large helper-tool executables. The release
workflow downloads known versions on a clean runner, verifies the Windows files
with SHA-256, packages the browser extension, and then builds native installers.

Run `.github/workflows/release.yml` manually to test the build, or push a version
tag such as `v0.4.4` to publish Windows `.exe` and `.msi` packages with a
`SHA256SUMS.txt` manifest and signed `latest.json` updater feed. Linux packaging is temporarily excluded from public
releases while its dependency compatibility is being updated.

Installed builds check the official GitHub release feed shortly after startup.
When a newer version is available, Velocity shows the version and release notes
and waits for the user to approve the download and installation. Update bundles
use Tauri's free cryptographic signatures even while the Windows installers
remain Authenticode-unsigned; this updater signature verifies integrity but does
not suppress Windows SmartScreen warnings.

## Downloads and Windows trust

Use only the [official GitHub Releases page](https://github.com/mr-kasim-mehar/velocity-download-manager/releases).
Compare the downloaded package against `SHA256SUMS.txt` before installing it.

Velocity is prepared to use free open-source signing from
[SignPath Foundation](https://signpath.org/). After the project is accepted and
the repository variables are configured, tagged GitHub Actions builds send the
Windows installers to SignPath and publish the returned Authenticode-signed
files. Until then, Windows packages are explicitly named `*-unsigned.exe` and
`*-unsigned.msi`; Windows may display an Unknown Publisher or SmartScreen
warning. See [the signing guide](docs/SIGNING.md) for the exact setup and
verification process.

## Browser companion

Velocity includes a Manifest V3 browser extension that displays a **Download this video** control on supported HTML5 video pages. The one-click overlay downloads the best available video, while the toolbar popup lets the user choose video resolution or audio-only format. The extension sends the page URL and validated preset through the registered `velocity://` protocol; the desktop app focuses the existing Velocity window and starts a normal media job.

Install the desktop app first, then open **Settings → Browser integration → Open package**. See [the browser integration guide](docs/BROWSER_INTEGRATION.md) for local Chrome, Edge, Opera GX, Brave, and Firefox instructions and the browser-store release process.

## Privacy, security, and license

Velocity contains no analytics, advertising, or telemetry. Read the
[privacy policy](PRIVACY.md) for the network behavior of media and torrent
downloads. Please report vulnerabilities according to the
[security policy](SECURITY.md), not in a public issue.

Velocity is licensed under the [MIT License](LICENSE). Bundled helper tools keep
their respective licenses; see
[`src-tauri/THIRD_PARTY_NOTICES.md`](src-tauri/THIRD_PARTY_NOTICES.md).

Developer: [Muhammad Qasim on Instagram](https://www.instagram.com/m_kasim2/)
