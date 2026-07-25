# Costa Utils

Desktop utility overlays for an Arch + Hyprland workstation — app launcher, clipboard
browser, screenshots, power / network / bluetooth / volume menus, and a control center.

This Rust workspace lives inside
[`arch-hyprland-installer`](https://github.com/tomascosta29/arch-hyprland-installer)
at `./costa-utils`. `install.sh` and `scripts/deploy-user` build and install it;
there is no Python fallback.

## Status

| Target | Status |
|--------|--------|
| `--power-menu` | Implemented |
| `--volume-menu` | Implemented (media card + artwork) |
| `--network-menu` | Implemented |
| `--bluetooth-menu` | Implemented (`bluetoothctl` + BlueZ pairing agent) |
| `--app-menu` / `--runner` | Implemented (arrow-key navigation) |
| `--clipper` | Implemented (pins, edit, JSON, path-open, preview/thumbs) |
| `--blinker` / `--blinker-area` / `--blinker-manager` | Implemented (manager settings + thumbs) |
| `--control-center` | Implemented (media artwork) |
| `--shutdown` | Quits the primary instance |

See [docs/MIGRATION.md](docs/MIGRATION.md) for the porting plan.

## Layout

```
crates/
  costa-core/   # targets, command runner, backends (no GTK)
  costa-ui/     # GTK4 + libadwaita windows
  costa-bin/    # `costa-utils` binary
assets/         # icons + desktop file
```

## Build

Needs Rust stable, `gtk4`, and `libadwaita` (Arch: `gtk4` `libadwaita` `base-devel`).

```bash
cargo build --release
./target/release/costa-utils --power-menu
```

Install locally:

```bash
cargo install --path crates/costa-bin
# optional:
# install -Dm644 assets/applications/org.fcosta.CostaUtils.desktop \
#   ~/.local/share/applications/org.fcosta.CostaUtils.desktop
```

Logs: `COSTA_UTILS_LOG_LEVEL=debug costa-utils --power-menu`

## Design notes

- **One process, many overlays** — `org.fcosta.CostaUtils` on the session bus; later
  invocations activate the existing instance.
- **UI-free core** — backends and CLI parsing stay testable without a display.
- **Feature-by-feature migration** — keep Python installed until each target is at parity.
- **Shared app id** — Rust and Python use the same D-Bus id, so only one primary can run.
  Quit the Python instance (or `costa-utils --shutdown`) before testing the Rust binary alone.
