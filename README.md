# AKP153E Open Controller

[Português](README.pt-BR.md) · [![CI](https://github.com/warleyvic24/akp153e-controller/actions/workflows/ci.yml/badge.svg)](https://github.com/warleyvic24/akp153e-controller/actions/workflows/ci.yml) · [MIT License](LICENSE)

Unofficial, open-source controller software for Windows and Linux for the AJAZZ AKP153E stream controller (15 LCD keys), written in Rust with egui/eframe. It replaces the manufacturer's app and contains no code, firmware or assets from it.

> **Status: early (v0.1).** Key drawing, the editor, app/webhook/shortcut actions, autostart and the tray icon are verified on a real device. Physical key press/release and long press are **not verified yet**; the side panel is a display without buttons on the AKP153E and is not used — see [`evals/hardware-checklist.md`](evals/hardware-checklist.md). This project is not affiliated with AJAZZ.

> **Not affiliated.** This is an independent project. It is not made, endorsed or supported by AJAZZ. "AJAZZ" and "AKP153E" are trademarks or product names of their respective owners and are used here only to say which device this software works with.

## Features

- **Editor** with a live preview of each key (the exact image the device shows), drag and drop, copy/paste, undo/redo, pages and profiles.
- **Actions:** keyboard shortcut, text, open app or website, shell command, webhook, media keys, multi-action with delays, page and profile navigation, volume mixer on the device, two-state keys and long press.
- **OBS Studio** (obs-websocket 5, built into OBS 28+): switch scene, stream, record, pause, mute an input, virtual camera, replay buffer. Keys follow OBS live: red while recording, streaming or muted; the current scene lights up.
- **Live keys:** a clock with time and date; volume keys show the current level (bar and %, red when muted); Play/Pause shows the album art, title and artist of what is playing (Linux, MPRIS); a system monitor key with CPU, RAM and temperature; **device** keys for brightness up and down.
- **Look:** 2,000 searchable icons ([Lucide](https://lucide.dev), search in English or Portuguese), your own PNG/JPG/SVG/ICO images (drop a file on a key), up to three label lines placed at the top, middle or bottom, optional gradient background, and a "Beautify page" button that gives the whole page category colors, gradients, labels and icons in one click (undoable).
- **Automatic profiles:** a profile switches in when its app gets focus (Windows, Hyprland, Sway and X11) and the previous one returns when you leave.
- **Profiles export/import** as one file with the key images inside.
- **Screen sleep** after a chosen idle time; the first press only wakes it.
- Dark, light and Omarchy themes; Portuguese and English.

## How it works

One executable, two modes:

| Mode | Command | Role |
|------|---------|------|
| Service | `akp153e --background` | Tray icon, no window. Owns USB/HID, actions and key drawing. |
| Editor | `akp153e --editor` | egui window. Edits `config.json` (`%APPDATA%\AJazz\AKP153E` on Windows, `~/.config/AJazz/AKP153E` on Linux) and tells the service to reload over `127.0.0.1:47153`. |

Other flags: `--probe` (writes `akp153e_probe.txt`), `--run-action <json>`, `--autostart on|off`.

The device enumerates as HID `0300:3010` and is handled as protocol v3. The protocol follows publicly available documentation; this project did no reverse engineering of the official app.

## Build

Requires a recent stable Rust toolchain (1.89 or newer) and:

- **Windows:** the MSVC build tools (the Windows SDK resource compiler embeds the icon).
- **Linux:** GTK 3, libayatana-appindicator, libudev, libxkbcommon and `pactl`; a udev rule gives your user access to the device. Packages, install script and what was tested (Omarchy / Hyprland) are in [`docs/LINUX.md`](docs/LINUX.md). On Arch: `cd packaging/arch && makepkg -si`.

```
cargo build --release   # target/release/akp153e (akp153e.exe on Windows)
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

On Windows, `cargo build --release` fails with "access denied" while the service is running; stop the `--background` process first.

## Security

The control port listens on localhost (on Linux, connections from other users are refused), and `config.json` stores webhook URLs, program paths and the OBS WebSocket password in plain text (owner-only on Linux). Details and reporting instructions are in [SECURITY.md](SECURITY.md).

## Contributing

Issues and pull requests are welcome. Read [`CLAUDE.md`](CLAUDE.md) for project conventions and [`docs/DESIGN.md`](docs/DESIGN.md) for the UI rules. CI runs formatting, clippy, and tests on every push.

## License

[MIT](LICENSE). Third-party licenses are in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md); regenerate it with `python tools/third_party_notices.py` after changing dependencies.
