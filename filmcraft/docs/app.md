# FilmCraft as an app (Capacitor) and on Netlify

The browser build (`apps/filmcraft-web`, see [web.md](web.md)) is the single source for both
targets: Netlify serves it as a website, Capacitor wraps the same files in a native WebView for
Android and iOS.

```
cargo xtask web  →  target/web/dist  →  web-dist/  ┬→  Netlify (publish dir)
                    (scripts/build-web.sh)          └→  Capacitor (webDir) → android/ , ios/
```

## Netlify

`netlify.toml` (repo root) sets base `filmcraft/`, runs `scripts/build-web.sh --ci` and publishes
`filmcraft/web-dist`. The script installs rustup, the `wasm32-unknown-unknown` target and the
`wasm-bindgen` CLI (prebuilt, version read from `xtask/src/main.rs`), then runs `cargo xtask web`.
`packaging/web/_headers` is copied into the output (`application/wasm`, immutable caching).

1. Netlify → *Add new site* → *Import from Git* → pick this repo. Settings come from `netlify.toml`.
2. The first build compiles the whole engine in release mode and takes a while; `CARGO_TARGET_DIR`
   points into Netlify's build cache so later builds are incremental.

Manual deploy from a machine with the toolchain: `npm run build:web && npx netlify deploy --prod --dir web-dist`.

## Capacitor (Android / iOS)

```sh
cd filmcraft
npm install
npm run cap:sync        # builds web-dist, copies it into android/ and ios/
npm run cap:android     # opens Android Studio
npm run cap:ios         # opens Xcode (macOS only)
```

`capacitor.config.json`: app id `ai.storyteller.filmcraft`, `webDir` = `web-dist`. `android/` and
`ios/` are committed; the copied web assets inside them are gitignored and recreated by `cap sync`.

## Phone layout

Windows narrower than 720 points (every phone in portrait) get a touch layout instead of the docked
desktop panels (`crates/ui-egui/src/mobile.rs`):

```
┌ Undo Redo   title   Export ┐
│  program monitor (picture) │
│  Start  Play  End   time   │
│  timeline (pinch to zoom)  │
│ + Video + Music Split Trim… │  ← scrollable tool row
└────────────────────────────┘
```

- **+ Video** opens the device's file picker (video, photos), imports the pick and appends it to the
  sequence. A new project gets a sequence with the first clip's size and frame rate.
- **+ Music** picks a local audio file and puts it at the playhead on its own audio track (a new
  one when the existing ones are busy), with Sync Lock off so picture edits do not move it.
- **Split** cuts at the playhead. **Trim in / Trim out** set the start or end of the selected clip
  (else the clip under the playhead) to the playhead, closing the gap when possible. **Delete**
  removes it. **Undo / Redo** are in the top bar. Tapping a clip selects it, dragging moves it.
- **Library** opens the Project bin as a sheet; select items and tap *Add to timeline*.
- **Export** is a single screen: file name, H.264 MP4 at the sequence size, one button.
  The result is a browser download.

Wider windows keep the desktop layout. The picker accepts `video/*`, `audio/*` and `image/*` as
well as the extension list, so phone browsers do not grey files out.

Things to know:

- **Renderer:** WebGPU is not available in most mobile WebViews, so the app falls back to WebGL2
  (automatic; `?webgl` forces it). Performance is below desktop.
- **Single-threaded wasm** and memory limits of mobile WebViews apply; large projects may run out
  of memory.
- **Files:** picked/dropped media are `File` handles via `<input type=file>` (File System Access
  API is not present in WebViews). Exports are offered as downloads; native share/save would need
  a Capacitor plugin (not included yet).
- The native projects have not been built or run on a device in this repo's CI.
