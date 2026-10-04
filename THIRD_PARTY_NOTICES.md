# Third-party software

Polotno uses the following open-source libraries. No Prism Mapper or react-projection-mapping source code has been copied into this prototype.

| Component | Upstream | License |
| --- | --- | --- |
| FFmpeg (standalone converter; LGPL configuration) | https://ffmpeg.org/ | LGPL-2.1-or-later |
| OpenH264 | https://github.com/cisco/openh264 | BSD-2-Clause |
| React, React DOM, Scheduler | https://github.com/facebook/react | MIT |
| AndroidX Media3 and AndroidX support dependencies | https://github.com/androidx/media | Apache-2.0 |
| ZXing Core | https://github.com/zxing/zxing | Apache-2.0 |
| Axum, Tokio, Tower and HTTP utilities | https://github.com/tokio-rs | MIT |
| Serde and serde_json | https://github.com/serde-rs | MIT OR Apache-2.0 |
| rusqlite | https://github.com/rusqlite/rusqlite | MIT |
| SQLite (bundled through libsqlite3-sys) | https://www.sqlite.org/copyright.html | Public domain |
| UUID | https://github.com/uuid-rs/uuid | MIT OR Apache-2.0 |
| JNI bindings | https://github.com/jni-rs/jni-rs | MIT OR Apache-2.0 |
| Reqwest and Tower HTTP | https://github.com/seanmonstar/reqwest / https://github.com/tower-rs/tower-http | MIT OR Apache-2.0 / MIT |

`Cargo.lock` pins the Rust dependency graph; Gradle pins the Android direct dependencies. Copyright/license texts and notices accompany distributed builds in the APK assets. Test-only libraries are not linked into the APK.

The converter is built from unmodified, checksum-pinned upstream source by `scripts/build-media-tools.py`. The corresponding FFmpeg and OpenH264 source URLs, configurations, and build steps are provided in that script; license texts accompany the APK. It runs as a separate process. No GPL/nonfree FFmpeg components or network protocols are enabled.
