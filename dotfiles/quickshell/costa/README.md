# costa Quickshell bar

Quickshell configuration shipped with the Arch Hyprland installer. Named shell
`costa` launches as `qs -c costa` under `quickshell.service`.

## Highlights

- Primary control bar with workspaces, system tray, session controls, and a dynamic center stage
- Secondary observability bar (dual-host) with active-window context, Git state, task detection, and telemetry
- VM / single-monitor profile collapses to one primary bar without GPU/temp sensors
- Theme colors come from the shared theme-pack `colors.css` via `theme-select`
- Optional IPC activity reporting through `qs-activity`

## Requirements

- Quickshell 0.3 or newer (`pacman -S quickshell`)
- Hyprland, `jq`, `git`, `procps-ng`, `pulseaudio-utils` (`pactl`), `iproute2`, `curl`
- JetBrainsMono Nerd Font
- `~/.local/bin/costa-utils` for launcher, clipboard, capture, control-center, network, Bluetooth, volume, and power

## Profiles

`quickshell-profile bare-metal|vm` writes `profile.json` and `costa/install-profile`.
Bare-metal expects `HDMI-A-1` as the primary monitor (session controls) and other
outputs as secondary (telemetry). VM / single mode treats the first screen as primary.

## Theming

`Colors.qml` watches `~/.config/quickshell/costa/colors.css`, which
`theme-select` points at the active pack's `colors.css`. The CSS
`@define-color` names are the single palette source; QML exposes them plus
aliases (`accent` → `soft-blue`, `backgroundAlt` → `background-alt1`, …).
