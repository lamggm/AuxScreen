# Third-party notices

AuxScreen is licensed under GPL-3.0-or-later. It depends on third-party software
whose licenses remain applicable to their respective components.

| Component | Version used by v0.1.0-rc.1 | License | Project |
|---|---:|---|---|
| zbus | 5.17.0 | MIT | https://github.com/dbus2/zbus |
| ashpd | 0.13.12 | MIT | https://github.com/bilelmoussaoui/ashpd |
| GStreamer Rust bindings | 0.25.3 | MIT | https://gitlab.freedesktop.org/gstreamer/gstreamer-rs |
| GStreamer / gst-plugins | 1.28.x | LGPL-2.1-or-later; individual plugins may differ | https://gstreamer.freedesktop.org |
| x264 | system package | GPL-2.0-or-later or commercial | https://www.videolan.org/developers/x264.html |
| AndroidX / Jetpack Compose | BOM 2026.06.00 | Apache-2.0 | https://developer.android.com/jetpack/androidx |
| OkHttp | 5.1.0 | Apache-2.0 | https://square.github.io/okhttp/ |
| WebRTC SDK for Android | 144.7559.09 | BSD-3-Clause and bundled notices | https://github.com/webrtc-sdk/android |

## Vendored zbus patch

`third_party/zbus-5.17.0` is the published MIT-licensed zbus 5.17.0 source with
four minimal Rust 1.97 reborrow corrections in
`src/connection/socket/command.rs`. Its original copyright and MIT license are
preserved in that directory. The patch does not change the public API.

Android packages also embed their own `META-INF` notices where supplied. A
release distributor must preserve those notices and audit the final dependency
graph; this debug prototype is not a public release artifact.
