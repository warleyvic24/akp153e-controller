# AKP153E Open Controller

[Português](README.pt-BR.md) · [![CI](https://github.com/warleyvic24/akp153e-controller/actions/workflows/ci.yml/badge.svg)](https://github.com/warleyvic24/akp153e-controller/actions/workflows/ci.yml) · [MIT License](LICENSE)

Unofficial, open-source, Windows-only controller software for the AJAZZ AKP153E stream controller (15 LCD keys), written in Rust with egui/eframe. It replaces the manufacturer's app and contains no code, firmware or assets from it.

> **Status: early (v0.1).** Key drawing, the editor, app/webhook/shortcut actions, autostart and the tray icon are verified on a real device. Physical key press/release and long press are **not verified yet**; the side panel is a display without buttons on the AKP153E and is not used — see [`evals/hardware-checklist.md`](evals/hardware-checklist.md). This project is not affiliated with AJAZZ.

> **Not affiliated.** This is an independent project. It is not made, endorsed or supported by AJAZZ. "AJAZZ" and "AKP153E" are trademarks or product names of their respective owners and are used here only to say which device this software works with.

## How it works

One executable, two modes:

| Mode | Command | Role |
|------|---------|------|
| Service | `akp153e.exe --background` | Tray icon, no window. Owns USB/HID, actions and key drawing. |
| Editor | `akp153e.exe --editor` | egui window. Edits `%APPDATA%\AJazz\AKP153E\config.json` and tells the service to reload over `127.0.0.1:47153`. |

Other flags: `--probe` (writes `akp153e_probe.txt`), `--run-action <json>`, `--autostart on|off`.

The device enumerates as HID `0300:3010` and is handled as protocol v3. The protocol follows publicly available documentation; this project did no reverse engineering of the official app.

## Build

Requires Windows, a recent stable Rust toolchain and the MSVC build tools (the Windows SDK resource compiler embeds the icon).

```
cargo build --release   # target\release\akp153e.exe
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

`cargo build --release` fails with "access denied" while the service is running; stop the `--background` process first.

## Security

The control port accepts any local process, and `config.json` stores webhook URLs and program paths in plain text. Details and reporting instructions are in [SECURITY.md](SECURITY.md).

## Contributing

Issues and pull requests are welcome. Read [`CLAUDE.md`](CLAUDE.md) for project conventions and [`docs/DESIGN.md`](docs/DESIGN.md) for the UI rules. CI runs formatting, clippy, and tests on every push.

## License

[MIT](LICENSE). Third-party licenses are in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md); regenerate it with `python tools/third_party_notices.py` after changing dependencies.
