# Полотно · Polotno

**Полотно** is the Russian product name. **Polotno** (`polotno`) is the international spelling and repository name.

A standalone, offline gallery for Android TV projectors. Arrange pictures and videos on your wall using a browser on your phone or computer. The projector stores and renders the media; the browser is a controller.

## Current status

The first technical prototype is implemented. It includes:

- Android TV launcher, remote controls, and a system `DreamService` screensaver.
- OpenGL ES rendering of a dynamic surface list with four-corner perspective mapping.
- Media3 video playback with no artificial video playlist, image, or surface count cap. Repeated active clips share a decoder and playback phase; each surface retains its own playlist selection clock. Decoder fallback is enabled, but smooth playback of distinct clips depends on device resources. Each surface supports independent horizontal and vertical reflection for images and videos, persisted with the scene. Video frame notifications are coalesced into one scene draw at up to 30 fps. Compatible H.264 SDR clips are remuxed without re-encoding, preserving lower frame rates for lighter playback.
- Rust/Axum embedded in the APK through JNI, SQLite scene storage, and private local media storage.
- Stable QR/code pairing, session cookies, and a Russian React editor with a visual frame, draggable regions/corners, live wall preview, playlists, and explicit saving.
- Ready-to-select original wallpapers: 12 seamless H.264 video loops (including cyberpunk) and 12 static modern abstractions, bundled for offline use. The Met paintings and museum search remain in a separate tab.
- Independent playlist clocks, pause/resume, and saved-scene restoration. Unsaved previews do not survive a restart.
- Rendering on demand for static scenes, shared image textures, sampled image decoding, and basic draw/memory/dropped-video-frame diagnostics. The browser uploads static textures once and updates each video texture only for a new decoded frame.

This is an early working version. Fades, backup/restore, further catalog expansion, and dedicated screensaver-scene selection are still pending. Uploaded files are streamed to local storage and normalized on the projector with Android ImageDecoder and a built-in FFmpeg/OpenH264 converter. Originals are retained. Uploads are limited to 512 MiB per file, separate from the unlimited surface count. Unsupported or protected codecs produce an error; no decoder can guarantee every proprietary format.

The target is the XGIMI MoGo 3 Pro G0035, Android 14, 1920×1080. Compilation and local tests do **not** establish physical playback, screensaver behavior, or one-hour stability. Initial physical findings are recorded in [HARDWARE_FINDINGS.md](docs/HARDWARE_FINDINGS.md); the remaining acceptance procedure is in [device validation](docs/DEVICE_VALIDATION.md).

## Build and test

Requirements: Node.js 22.12+ and npm, Python 3 (license bundling), make, JDK 17 or 21, Rust 1.91.1 or later, Android SDK platform/build-tools 35, NDK `27.0.12077973`, and the Android Rust targets:

```sh
rustup target add armv7-linux-androideabi aarch64-linux-android x86_64-linux-android
export ANDROID_HOME=/path/to/android-sdk
npm --prefix web ci
npm --prefix web run build
npm --prefix web test
npm --prefix web run test:e2e
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
./gradlew :android:app:assembleDebug :android:app:testDebugUnitTest :android:app:lintDebug
```

The Gradle build compiles Rust for ARMv7/ARM64 devices and x86_64 emulators. The physical MoGo reports **armeabi-v7a only**, despite its 64-bit-capable hardware; the ARMv7 library is required. The debug APK is written to `android/app/build/outputs/apk/debug/app-debug.apk`. Install only on an explicitly selected device, using an update install (`adb -s DEVICE install -r ...`). Do not uninstall the app to resolve an update-signature mismatch.

The debug key is for development only. Before persistent personal use, configure a permanent signing key outside iCloud, retain it for updates, and back up gallery data. No release signing keys are stored in this repository.

To exercise the server without Android:

```sh
cargo run --locked -p polotno-core --bin polotno-dev -- runtime
```

Open the pairing URL printed in your local terminal. Enter stops the server. This development runner binds port 8787; Android uses port 8787 when available, falling back to an available port if another service occupies it. `runtime/` contains local data and is ignored by Git. Pairing uses plain HTTP on a trusted local network; keep this prototype off the public internet.

## Hardware prototype workflow

1. Launch **Полотно** from the projector's app list. Scan its QR code from the same Wi-Fi network. The installation keeps the same QR and six-digit code across Menu, pairing, and app restarts. A computer can also open the base address and enter the six-digit code shown on the projector. Browser pairing lasts 30 days and survives app restarts.
2. Pick an offline wallpaper from **Динамические** or **Модерн**, or a museum painting from **Картины**, and press **Показать**. Museum paintings download directly to the projector; local file uploads are optional under **Мои файлы**.
3. Drag a region to move it, or drag its four corners to change the perspective. Changes appear on the projector automatically. Add, duplicate, remove, and reorder regions through **Области**.
4. Choose **Сохранить сцену** to persist the composition. Creating a new scene starts with no surfaces and a black frame.
5. After pairing, the projector hides the QR panel and shows the scene. Menu toggles the panel, arrows switch saved scenes, OK pauses/resumes playlists, and Back exits. Players, GL resources, and the embedded server are released when the gallery/screensaver closes.

Select Polotno as a screensaver through the device's normal Android settings. The app does not rewrite system sleep, brightness, Wi-Fi, HDMI, or account settings. Android decides when the screensaver starts and dismisses it on remote input.

## Layout

- `core/`: Rust model, SQLite store, upload/pairing APIs, JNI, local server runner, and contract tests.
- `android/app/`: Kotlin lifecycle, remote input, GLES rendering, Media3, and screensaver.
- `web/`: TypeScript/React visual editor, projective browser preview, museum collection, and browser tests.
- `scripts/build-native.sh`: NDK cross-compilation.
- [PLAN.md](PLAN.md): implementation plan (Russian).
- [AGENTS.md](AGENTS.md): project contracts.

## License

[MIT](LICENSE). Dependency attribution is in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). This independent project is not affiliated with XGIMI or Dil Studio.

The Met integration uses its current paginated `v1.1/search` endpoint. Art Institute of Chicago is also supported by the API (`provider=aic`), but its image CDN was unavailable during the initial check; the working browser collection defaults to The Met.

Saved browser scenes are available as one-click composition cards beside **Новая сцена**. Drag updates are coalesced per browser frame and streamed in order during movement. Immutable local media can be cached privately by the browser.

The **Динамические** tab is the default. **Киберпанк**, **Спокойные**, and **Абстракция** filter the ready collection without a search. Only one catalog thumbnail plays at a time. Browser scene video textures use a lightweight preview; projector playback retains the original media. Original wallpapers can be regenerated with `python3 scripts/generate-wallpapers.py` (FFmpeg required).

New regions start with a calibration grid visible on the wall. Selecting media replaces the grid. **Целиком**, **Заполнить**, and **Растянуть** control aspect handling. Each region can have an independently colored neon outline with thickness and glow controls. Uploaded video cards have generated still previews and an on-demand moving preview.
