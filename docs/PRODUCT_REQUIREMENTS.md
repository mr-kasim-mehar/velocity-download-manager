# Velocity Download Manager — Product Requirements

## 1. Product scope

Velocity is a native Windows and Linux desktop download manager for direct HTTP(S) files, supported media pages, magnet links, and BitTorrent metadata files. It must remain responsive during large transfers, recover safely from interruption, and require minimal setup after installation.

The application does not bypass DRM, paywalls, authentication, or access controls. Users are responsible for downloading only content they are authorized to save.

## 2. Functional requirements (FR)

### FR-01 — Add a source

- Accept HTTP and HTTPS direct-file URLs.
- Accept supported media-page URLs, including YouTube URLs, through the media engine.
- Accept `magnet:` URIs.
- Import local `.torrent` files from a native file picker.
- Detect common source types automatically and allow an explicit source type when detection is ambiguous.
- Validate malformed or unsupported input before creating a job.

### FR-02 — Destination management

- Default all new jobs to the operating system Downloads folder.
- Allow the default download folder to be changed in Settings and persist it across restarts.
- Allow a destination override for each new job.
- Create a missing destination directory when possible and show an actionable error otherwise.

### FR-03 — Direct-file transfers

- Stream HTTP(S) response bodies directly to disk.
- Resume partial files when the server supports byte ranges.
- Follow a bounded number of redirects.
- Preserve partial data on pause, network failure, and app shutdown.
- Show transferred bytes, total size when known, instantaneous speed, progress, and ETA.

### FR-04 — Media-page transfers

- Use the maintained `yt-dlp` extractor engine for supported websites.
- Auto-detect common video and social platforms while retaining yt-dlp's generic extractor for other supported pages.
- Clearly report authentication, site-change, unavailable-content, and unsupported/DRM errors; do not claim universal website support.
- Default to a broadly compatible single-file format when FFmpeg is unavailable.
- Display the resolved media filename and extractor errors.
- Let users choose Best, 2160p, 1440p, 1080p, 720p, 480p, or 360p as a maximum video resolution, falling back to the best source at or below the limit.
- Support audio-only extraction to MP3, M4A, AAC, Opus, FLAC, and WAV through validated presets.
- Persist the selected media mode and format with the job so pause, resume, and retry retain the user's choice.
- Support pause/cancel by stopping the worker process and allow a clean retry.
- Never construct shell command strings from user input; pass every argument separately.

### FR-05 — BitTorrent transfers

- Use `aria2c` for magnet and `.torrent` jobs.
- Enable DHT, peer exchange, tracker operation, multi-file torrents, and resume metadata.
- Stop seeding when the requested download completes by default.
- Show torrent status, speed, peer information when available, and the resolved torrent name.
- Preserve `.aria2` control files so interrupted jobs can resume.
- Keep incomplete torrent content and `.aria2` resume metadata in a hidden, per-job staging directory on the selected drive.
- Publish only completed torrent files or folders into the user-visible destination.
- Explain the operating-system firewall prompt before the first torrent starts and recommend Private-network access only.

### FR-06 — Queue and lifecycle

- Persist job metadata and restore incomplete jobs as paused after restart.
- Support pause, resume/retry, remove-from-history, open, and reveal-in-folder.
- Support pausing all active jobs and clearing completed history.
- Separate active, completed, and failed views and support text search.
- Do not delete downloaded or partial files when merely removing history unless the user explicitly requests deletion.

### FR-07 — User feedback

- Use distinct queued, resolving, downloading, paused, completed, and failed states.
- Provide human-readable errors for network, storage, extractor, and torrent-engine failures.
- Show when a required bundled engine is unavailable or damaged.
- Avoid reporting completion until the final output exists on disk.

### FR-08 — Packaging

- Produce signed-capable NSIS `.exe` and WiX `.msi` packages on Windows.
- Produce `.deb` and AppImage packages on Linux.
- Bundle the platform-appropriate media and torrent workers or install declared package dependencies.
- Keep worker licenses and third-party notices in the distributed package.

### FR-09 — Browser integration

- Provide a Manifest V3 companion extension for Chromium browsers and Firefox.
- Display a restrained Download this video overlay on sufficiently large HTML5 video players.
- Provide the same action from the browser toolbar for supported pages where an overlay cannot be injected.
- Hand off only validated HTTP(S) page URLs through the registered `velocity://` desktop protocol.
- Focus the existing Velocity window instead of opening duplicate application instances.
- Ignore duplicate handoffs generated within five seconds.
- Do not read cookies, bypass DRM, or claim support for protected or inaccessible content.

## 3. Non-functional requirements (NFR)

### NFR-01 — Memory efficiency (priority)

- Stream network data in bounded chunks; never load a complete download into memory.
- Keep only job metadata and a small progress window in the UI process.
- Keep UI event payloads compact and emit progress no more than a few times per second.
- Run media/torrent engines only while their jobs are active.
- Target idle resident memory below 100 MB on a typical Windows installation.
- Target incremental memory below 15 MB per direct-file transfer, excluding OS disk cache and external worker processes.
- Virtualize or page the history view before supporting thousands of visible jobs.

### NFR-02 — Performance

- Keep UI interactions responsive while transfers are active.
- Begin a validated direct download within two seconds on a normal connection.
- Avoid synchronous disk or network work on the UI thread.
- Use native streaming and bounded update frequency to reduce CPU wakeups.

### NFR-03 — Reliability

- Write direct downloads to a distinct partial filename and rename atomically on completion.
- Stage media fragments and merge intermediates in the application cache, not the user's destination folder.
- Wait for an active worker to stop before removing its job or cleaning its temporary files.
- Persist state after material lifecycle changes.
- Limit redirects and recover from transient network failure without corrupting completed data.
- Sanitize filenames for the target operating system.

### NFR-04 — Security

- Accept only HTTP(S), magnet URIs, and user-selected torrent files.
- Pass worker arguments without invoking a command shell.
- Do not enable unsafe extractor compatibility options.
- Restrict desktop permissions to the minimum file-dialog/open/reveal capabilities.
- Verify hashes for bundled third-party executables in the release pipeline.

### NFR-05 — Privacy

- Store history and settings locally.
- Include no telemetry by default.
- Do not upload URLs, torrent metadata, or filenames to a Velocity service.

### NFR-06 — Accessibility and UX

- Provide keyboard-focus states, meaningful labels, adequate contrast, and non-color status cues.
- Use native terminology, restrained visual styling, and consistent controls.
- Keep the primary add flow understandable without configuration.

### NFR-07 — Maintainability

- Keep transfer engines behind source-specific adapters.
- Keep persisted data backward-compatible through serde defaults and versioned settings.
- Pin release-engine versions and document their licenses.
- Cover URL classification, filename sanitization, settings, and progress parsing with automated tests.

## 4. Acceptance criteria for the current milestone

1. A direct file URL downloads into the default Downloads directory and can be paused and resumed.
2. A YouTube URL downloads through the bundled media worker at the requested maximum resolution or audio format, reports byte speed and percentage, and reports a meaningful failure when the source is unavailable.
3. A magnet link and a local `.torrent` file start through the bundled torrent worker and report total bytes, progress, speed, connected peers, and tracker/engine failures.
4. A user can change and persist the default destination, or override it for one job.
5. Closing and reopening Velocity preserves history and restores incomplete work as paused.
6. The Windows release launches without a terminal window.
7. Production frontend build, Rust tests, and Windows installer builds pass.

## 5. Future milestones

- Torrent file selection, priorities, peer/tracker details, and bandwidth limits.
- Configurable concurrent-job limits and scheduling.
- Browser extension integration.
- Checksums, proxy configuration, cookies/auth import, and signed auto-updates.
- Code signing and reproducible third-party binary verification in CI.
