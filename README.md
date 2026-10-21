# AJAZZ AKP153E Controller

[Português](README.pt-BR.md) · [![CI](https://github.com/warleyvic24/akp153e-controller/actions/workflows/ci.yml/badge.svg)](https://github.com/warleyvic24/akp153e-controller/actions/workflows/ci.yml) · [MIT License](LICENSE)

An open-source, Windows-only replacement for the official AJAZZ AKP153E app (a stream controller with 15 LCD keys), written in Rust with egui/eframe.

> **Status: early (v0.1).** Key drawing, the editor, app/webhook/shortcut actions, autostart and the tray icon are verified on a real device. Physical key press/release, long press and the 3 side keys are **not verified yet** — see [`evals/hardware-checklist.md`](evals/hardware-checklist.md). This project is not affiliated with AJAZZ.

## How it works

One executable, two modes:

| Mode | Command | Role |
|------|---------|------|
| Service | `akp153e.exe --background` | Tray icon, no window. Owns USB/HID, actions and key drawing. |
| Editor | `akp153e.exe --editor` | egui window. Edits `%APPDATA%\AJazz\AKP153E\config.json` and tells the service to reload over `127.0.0.1:47153`. |

Other flags: `--probe` (writes `akp153e_probe.txt`), `--run-action <json>`, `--autostart on|off`.

The device enumerates as HID `0300:3010` and is handled as protocol v3. The protocol comes from public documentation, not from our own reverse engineering.

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

[MIT](LICENSE)
