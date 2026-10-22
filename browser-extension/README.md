# Velocity Download Companion

The companion adds a **Download this video** control to webpages containing a sufficiently large HTML5 video. It sends the page URL—not private cookies or page content—to the installed Velocity desktop application through the `velocity://` protocol.

## Local installation

### Chrome, Edge, Brave, Opera and Opera GX

1. Install and launch Velocity once so the `velocity://` protocol is registered.
2. Open the browser's extensions page (`chrome://extensions`, `edge://extensions`, or `opera://extensions`).
3. Enable **Developer mode**.
4. Choose **Load unpacked** and select this `browser-extension` directory.
5. Open a supported video page and click **Download this video** over the player for the best video, or use the toolbar icon to choose a maximum resolution or an audio-only format.
6. On the browser's first handoff, approve **Open Velocity** and enable the browser's remember option if offered.

### Firefox development testing

1. Install and launch Velocity once.
2. Open `about:debugging#/runtime/this-firefox`.
3. Choose **Load Temporary Add-on** and select `manifest.json`.

Release Firefox requires a Mozilla-signed XPI. Chrome/Edge/Opera production distribution should use their extension stores rather than asking normal users to enable Developer mode.

## Security and behavior

- Only `http://` and `https://` page URLs are accepted by the desktop app.
- Duplicate browser events within five seconds are ignored.
- The extension does not read browser cookies or bypass DRM, authentication, paywalls, or access controls.
- Known DRM subscription services do not receive an overlay.
- A site supported by yt-dlp can still temporarily fail when the site changes or requires authentication.
