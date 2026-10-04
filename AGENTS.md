# Полотно (Polotno) — development rules

Read PLAN.md before implementation. This is a standalone software project, not part of the Aleph vault.

## Product contracts

- Surface count is dynamic. Never hard-code three regions or replace that with another arbitrary image-count cap.
- Video decoder limits are separate: at most two video-bearing surface playlists per scene in v1.
- The projector renders and stores media locally. A phone or computer is a browser controller, not a required streaming source.
- Kotlin handles Android lifecycle, remote input, DreamService and Media3. Rust is embedded in the APK; the web editor uses TypeScript and React.
- Keep the user interface in Russian. QR pairing is the primary setup path.
- Follow provider-specific requirements. Do not implement an unofficial Unsplash API cache or scraping as a substitute for an approved integration.

## Device and data

- Never factory-reset, root, flash firmware, clear another application's data, or uninstall another application as part of setup or verification.
- Preserve Android system settings and accounts. Do not alter power protections, global brightness, Wi-Fi or HDMI configuration to make tests pass.
- Keep signing keys, credentials, private media and device diagnostics out of Git. Store signing keys outside iCloud and reuse the same key for updates.
- Export or back up gallery data before any migration or destructive gallery operation.

## Verification

- Test scene persistence, independent playlists, surface mutations, pairing, uploads and resource cleanup with meaningful tests.
- Distinguish compilation/emulator results from physical MoGo validation. Never report device playback, screensaver behavior or an hour-long soak as verified without actually checking it.
- Validate the two-video path and native graphics before building the full catalog and editor. Log hardware capability findings without personal identifiers or media.
- Avoid continuously rendering static scenes when no visual state changes. Measure memory, frame pacing and dropped video frames on the target device.
- Reuse compatible open-source code with its required attribution and license notices.
