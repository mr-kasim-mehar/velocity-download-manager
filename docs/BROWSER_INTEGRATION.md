# Velocity browser integration

## User experience

When a webpage contains a visible HTML5 video player larger than the minimum overlay threshold, the Velocity companion places a compact **Download this video** control at the player's top-right corner. The control follows the player while scrolling, survives single-page navigation such as YouTube route changes, and works in fullscreen. Users can dismiss it for the current page. A toolbar action is available as a fallback.

Known DRM subscription services are excluded. The button sends the page URL to Velocity; it does not scrape cookies, inspect private page data, or download media inside the browser.

## End-to-end flow

```text
Video webpage
    ↓ user click
Velocity Manifest V3 content script
    ↓ velocity://download?url=<encoded HTTPS URL>&type=media&mode=<video|audio>&resolution=<preset>&audio_format=<preset>
Operating-system protocol handler
    ↓ opens or focuses one Velocity process
Validated desktop handoff
    ↓ normal media job
yt-dlp → FFmpeg merge → selected Downloads folder
```

1. The content script searches for the largest visible `<video>` element and positions its isolated Shadow DOM overlay without changing the website's CSS.
2. A real user click launches the registered `velocity://` URL. Browsers normally show an external-application confirmation the first time.
3. The Windows/Linux protocol registration starts Velocity. The single-instance plugin forwards later links to the running process.
4. Velocity accepts only the `velocity://download` route and only an embedded `http://` or `https://` URL. Unknown routes, local files, commands, and malformed URLs are rejected.
5. Repeated handoffs for the same URL within five seconds are ignored.
6. The in-player button uses best-available video. The toolbar popup can instead request a maximum resolution or MP3/M4A/AAC/Opus/FLAC/WAV audio output.
7. The page URL enters the existing media engine, so progress, pause, errors, cache staging, and destination settings work exactly like a URL pasted into the app.

## Local installation and testing

The desktop installer includes `browser-extension.zip`. In Velocity, open **Settings → Browser integration → Open package**, extract it, then follow `README.md` inside the package.

- Chromium browsers: enable Developer mode on the extensions page and choose **Load unpacked**.
- Firefox development: use `about:debugging#/runtime/this-firefox` and load `manifest.json` as a temporary add-on.
- Launch Velocity once before testing so the desktop protocol is registered.
- Visit a supported video page, click the overlay, and approve the browser's **Open Velocity** prompt.

## Production distribution

Normal users should not be asked to enable Developer mode. Release distribution requires separate browser-store submissions:

1. Create store accounts and privacy/support pages.
2. Zip the extension with `manifest.json` at the archive root.
3. Submit the same audited Manifest V3 source to Chrome Web Store and Microsoft Edge Add-ons; submit to Opera Add-ons for an Opera-specific listing if desired.
4. Submit to Mozilla Add-ons (AMO). Firefox release builds require Mozilla signing, including self-distributed builds.
5. Explain the broad HTTP(S) host permission: it is required only to detect video players and insert the button. The extension sends data only after a user click.
6. Publish updates through each store; Manifest V3 does not permit remotely hosted extension code.

## Limitations and next hardening step

- Website changes, login-only media, removed content, and DRM can still fail in yt-dlp.
- `blob:` and MediaSource URLs are intentionally not captured; the canonical page URL is sent to the extractor.
- Browsers may display a one-time external-protocol confirmation. A future store release can replace the protocol handoff with a signed Native Messaging host for a zero-prompt experience, but that requires stable store extension IDs and per-browser host registration in the installer.
