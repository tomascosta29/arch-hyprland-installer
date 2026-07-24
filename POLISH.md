# Technical polish review

Reviewed: 2026-07-24

## Implementation status

The substantive items from this review were implemented on 2026-07-24:

- Costa Utils now owns bounded jobs plus shared audio, media, NetworkManager,
  BlueZ, night-light, command, and path backends. Media monitoring and artwork
  caching are shared across windows; stale keyed work is discarded.
- Bluetooth has adapter discovery, a BlueZ `Agent1`, pair/trust/connect,
  disconnect, cancellation, forget, bounded discovery, and property-change
  refreshes. Wi-Fi carries BSSID/security/profile UUID state, handles open and
  personal networks distinctly, and hands enterprise setup to `nmtui`.
- Clipper decoding and mutations are asynchronous, preview generations are
  guarded, caches are byte/entry-bounded LRUs, and PNG/JPEG/GIF/WebP share the
  image decoder.
- Audio and brightness sliders are debounced. Manual Hyprsunset state is
  explicit instead of being inferred from the scheduled profile.
- Runtime theme selection validates first and switches every consumer through
  one atomic `current-theme` symlink. Supervised services, Kitty, and the Costa
  Utils singleton are notified; SDDM is explicitly installation-time only.
- User deployment records exact ownership, removes only stale managed files,
  preserves unowned user files, and shuts Costa Utils down through its
  application protocol.
- The installer performs clock/mirror preflight before passwords or destructive
  confirmation and discovers new partitions through `lsblk` partition numbers.
- Waybar selects VM or bare-metal telemetry explicitly, and an installed-system
  validator plus QEMU Guest Agent smoke harness now cover the deployed machine.

The discussion below is retained as the rationale and regression checklist, not
as a list of currently open tasks.

This review deliberately excludes repository cosmetics, licensing, screenshots,
badges, changelogs, and similar maintenance. It focuses on the behavior and
architecture of the installer, session configuration, switchers, Waybar, SDDM,
and Costa Utils.

## Original expert opinion

The repository has moved past basic repair. The installer has sensible
destructive-operation safeguards, the desktop stack is coherent, and the Lua
migration is substantially correct. The remaining polish is not another round
of formatting or small guards. It is about giving Costa Utils a real backend
architecture, making stateful operations cancellable, and hardening deployment
and installation as transactions.

The highest-leverage remaining change is to separate Costa Utils' system
backends from its GTK windows, with bounded and cancellable background work.
That would address most of the remaining silent failures, races, duplicated
code, and difficult-to-test behavior.

## Priority 1: give Costa Utils a backend and job model

Costa Utils is a long-lived singleton process containing roughly 5,800 lines of
module code. Its windows currently own subprocess execution, D-Bus calls,
parsing, caching, network requests, threading, state mutation, and GTK rendering.
That structure is now the limiting factor, not code style.

### Extract shared system adapters

Create focused backends for:

- PipeWire/PulseAudio;
- MPRIS/player state and artwork;
- NetworkManager;
- BlueZ;
- Hyprsunset;
- Dunst;
- clipboard history;
- screenshot storage.

The Control Center and Volume window currently duplicate the complete MPRIS
monitor and artwork loader. Network and Bluetooth state are implemented again
inside the Control Center instead of sharing the menu backends. Fixes therefore
have to be applied more than once and can produce different state interpretations
in different windows.

GTK windows should receive typed state and invoke backend operations. Parsing and
system interaction should not live inside button callbacks.

### Replace thread-per-event with bounded, cancellable work

Many UI actions create a new daemon thread immediately. The volume and
brightness sliders create a thread for every `value-changed` event. Dragging a
slider can enqueue dozens of `wpctl` or `brightnessctl` processes, and completion
order is not guaranteed. An older command can land after the final value.

Refresh buttons can also start overlapping scans. There is no request generation
or cancellation token, so a stale Wi-Fi, Bluetooth, audio, or artwork result can
replace a newer one.

Use a small executor or Gio async APIs, with:

- one in-flight refresh per backend;
- generation IDs so stale results are ignored;
- debounce/coalescing for sliders;
- cancellation when a window hides;
- bounded subprocess timeouts;
- explicit success and failure results.

### Add real observability

The modules contain 57 exception handlers, with many broad
`except Exception` branches and numerous silent `pass` paths. In a daemon
normally launched by a keybinding, `print()` is not a useful operational
interface either.

Use Python logging so messages reach the user journal, and reserve silent
handling for genuinely expected cancellation. Backend errors should produce a
structured result that the window can show as a toast. This is more valuable
than adding further defensive `try/except` blocks.

### Keep the D-Bus singleton, but make it the application boundary

The existing `org.freedesktop.Application` forwarding is a good foundation.
The next step is to let the singleton own shared backends and subscriptions, not
merely a collection of permanently retained window objects. This prevents every
window from starting its own monitors and makes state consistent across the
Control Center, volume menu, Bluetooth menu, and network menu.

## Concrete functional issues

### Bluetooth can discover devices but is not a complete device manager

[`bluetooth_menu.py`](dotfiles/costa-utils/costautils/bluetooth_menu.py) has no
BlueZ `Agent1`, `Pair`, `Trust`, `CancelPairing`, or forget-device flow. Calling
`Device1.Connect` is enough for an already paired device and some simple devices,
but it cannot reliably onboard devices that require confirmation, a PIN, or
authorization.

Discovery is started on every refresh and never stopped. Closing the window does
not call `StopDiscovery`, so the adapter can remain scanning after the UI has
disappeared. The code also begins with a hard-coded `/org/bluez/hci0` fallback
and treats the adapter as powered until a query proves otherwise, which gives
poor behavior on systems with no adapter or a differently named adapter.

A polished Bluetooth backend should:

- discover the adapter from ObjectManager;
- represent “no adapter” separately from “powered off”;
- own a BlueZ pairing agent;
- expose pair, trust, connect, disconnect, and remove operations;
- stop discovery on timeout or when the last consumer closes;
- subscribe to `PropertiesChanged` instead of rescanning after fixed sleeps.

### The Network menu only models saved profiles and WPA-like password entry

The scan requests `SSID,SIGNAL,ACTIVE,BARS` but not `SECURITY`. Every unsaved
network therefore takes the same password-prompt path. Open networks have to pass
through a password UI, while enterprise, captive-portal, and more complex
NetworkManager profiles cannot be configured correctly.

The saved-profile lookup by SSID is a useful repair, but SSID is not a unique
identity. Multiple profiles or access points can share it. The backend should
carry BSSID, security capabilities, connection UUID, and device name, and use
NetworkManager's D-Bus API or a better-defined `nmcli` contract.

`refresh_networks()` also calls the five-second `nmcli radio wifi` query on the
GTK thread before it starts its worker. That query belongs in the same
asynchronous refresh operation.

### Clipper performs expensive decode work on the GTK main loop

Clipper schedules `load_thumbs_idle`, but that idle callback calls
`cliphist decode` synchronously. It can decode up to 100 entries, one main-loop
iteration at a time. Selection preview decoding is also synchronous. Large image
entries can therefore make the UI visibly hitch even though the work is called
from an “idle” callback.

The singleton retains both raw decoded data and textures in unbounded
dictionaries. Reloading prunes neither cache unless the user performs a full
wipe. Because the application intentionally stays alive for the whole session,
clipboard images can permanently increase its memory footprint.

Move decoding and image scaling off the GTK thread, use an LRU bounded by bytes,
and remove entries no longer present in `cliphist list`. The MIME detector knows
about GIF and WebP, but thumbnail rendering only accepts PNG and JPEG; those
paths should use one shared decoder.

### Media monitoring has duplicated parsing and race behavior

Both the Volume window and Control Center parse `playerctl -F` output using
`::` as a delimiter. Titles and artist names can contain that sequence, causing
fields to be misassigned. Use a machine-safe format such as JSON or NUL-separated
fields.

Artwork requests are started independently for every metadata change. A slow
response from the previous track can overwrite the current track's artwork.
There is no URL/result generation check or shared cache. `file://` URLs are also
sliced manually instead of URI-decoded, so encoded spaces and other characters
can fail.

One MPRIS backend should own the follower process, parse structured records,
cache bounded artwork, and publish track generations to both windows.

### Night-light state does not match the action it controls

The Control Center enables and disables night light with the Hyprsunset
`temperature` and `identity` overrides, but determines button state by asking
for the active scheduled profile. An identity daytime profile remains the
current profile after a manual temperature override, and a warm nighttime
profile remains current after a manual identity override. The button can
therefore immediately report the opposite of the actual override.

Model scheduled state and manual override separately, or make the button reset
to the schedule rather than pretending the active profile describes the current
manual filter.

### Theme switching is file-atomic, not transaction-atomic or fully live

[`theme-select`](dotfiles/scripts/theme-select) safely replaces each destination
file, but the theme as a whole is not atomic. A failure halfway through leaves a
valid but mixed theme. Validation checks file presence only; it does not validate
the color schema or syntax before changing live files.

The live-application claim is also incomplete:

- Hyprland, Waybar, Dunst, and Hyprpaper receive some form of reload;
- existing Kitty windows are not reloaded;
- existing GTK applications and the long-lived Costa Utils daemon are not
  explicitly reloaded;
- SDDM is updated only when the normal desktop user can write to
  `/usr/share/sddm/themes/costa`, which is normally never after installation.

Stage and validate a complete runtime theme directory, switch one `current`
symlink or manifest, then notify supervised services. Decide explicitly whether
SDDM is part of runtime theme switching; if it is, it needs a narrow privileged
helper rather than a usually skipped write.

### `deploy-user` is not equivalent to the user portion of installation

The deployer overlays directories with `cp -a`, leaving files that were removed
from the repository in the live configuration. It then kills Costa Utils using
two process-pattern matches without a protocol-level shutdown.

Use one shared deployment manifest for the installer and deployer. Stage the
managed files, validate them, synchronize only the owned paths, and restart the
application through D-Bus or a user unit. User-created files outside the manifest
should remain untouched.

### Installer device handling should derive partitions, not guess suffixes

The installer constructs partition paths by adding `p1/p2` only for device names
containing `nvme` or `mmcblk`, and `1/2` for everything else. That covers the
expected SATA and VirtIO disks but unnecessarily bakes Linux device naming into
the destructive path. Other whole-disk types can require `p` separators.

After partitioning, query `lsblk` for the child partitions and identify them by
partition number/type or label. This also lets the installer verify that exactly
the intended EFI and root partitions appeared before formatting.

`os-prober` is intentional: the supported bare-metal layout may keep Windows on
a separate physical disk while dedicating the selected disk to Arch. Testing
should cover discovery of that untouched Windows EFI installation as well as
the single-disk VM case.

Finally, add preflight checks for repository/network reachability and clock
health before the partition table is replaced. A `pacstrap` failure is safe in
the data-integrity sense, but discovering network or time problems after wiping
the disk is poor installation behavior.

### Waybar has configuration that disagrees with its visible bar

The static usage drawer exposes AMD GPU and VRAM modules in the VM, where they
intentionally return zero. A machine/profile-aware module list would be cleaner
than showing plausible-looking zero telemetry for nonexistent hardware.

Waybar workspace clicking should be explicitly tested with the supported
Hyprland Lua/Waybar combination. There is current upstream compatibility work
around the workspace module dispatching old-style commands into Lua-based
Hyprland:
<https://github.com/Alexays/Waybar/issues/5008>.

### Screenshot directory paths are not normalized once

Normalize the configured screenshot directory once in a shared backend so the
launcher and manager cannot interpret relative paths differently.

## Validation strategy that matches the actual risks

The existing tests are useful regression guards, but most installer and
configuration tests assert that particular strings exist. The validation script
does not currently parse or exercise:

- SDDM QML;
- Hyprlock, Hypridle, Hyprsunset, Dunst, Rofi, or Kitty configuration;
- Hyprland's runtime Lua types;
- Waybar/Hyprland workspace interaction;
- Costa Utils window construction and backend failure paths;
- the destructive installer against a disposable disk.

The next test investment should follow the architecture:

1. pure tests for backend parsers and state transitions;
2. mocked adapter tests for timeouts, stale generations, and errors;
3. headless GTK smoke tests that construct and activate every window;
4. config validators invoked from the target Arch environment;
5. a manual or scheduled disposable-VM installation test covering partitioning,
   boot, enabled services, session startup, and `hyprctl configerrors`.

The VM was inspected during this review, but it is currently booted into the
Arch installation ISO (`archiso`) and has no installed `fcosta` user or desktop
packages. It therefore could not provide empirical validation of the deployed
Hyprland session. That missing live check is itself the strongest argument for
keeping a repeatable installed-guest smoke test.

## Recommended implementation order

1. Extract shared audio, MPRIS, NetworkManager, BlueZ, and command-runner
   backends.
2. Add bounded jobs, cancellation/generations, and logging.
3. Fix Bluetooth onboarding, Clipper caching/decoding, and night-light state.
4. Make theme and user deployment manifest-driven and transaction-oriented.
5. Harden installer partition discovery and pre-wipe preflight checks.
6. Add the installed-VM smoke test now that the service graph is stable.

---

## Archived (done)

Resolved on 2026-07-24. This includes both architectural work and smaller
repairs; do not re-open an item unless its fix regresses.

### Desktop session lifecycle is supervised

`hyprland-session.target` now binds to `graphical-session.target`, starts XDG
autostart, and owns the packaged Hyprpaper, Hypridle, Hyprsunset, Waybar, Dunst,
and policy-agent services plus dedicated clipboard watcher services. Drop-ins
bind every process to the session and give it restart-on-failure behavior.
Hyprland imports its environment, starts the target on `hyprland.start`, and
stops it synchronously on `hyprland.shutdown`.

### Hyprland configuration is Lua-only

The repository's Hyprland 0.55+ boundary is now real. The handwritten Hyprlang
main, color, input, and monitor mirrors were removed. Theme, monitor, and
desktop-settings scripts mutate only Lua state, and deployment removes retired
mirrors from existing installations.

### Exit path uses hyprshutdown

`SUPER+SHIFT+M` now runs `hyprshutdown` (with a raw-exit fallback). The
installer installs the `hyprshutdown` package from `extra`.

### SDDM clock advances; root follows the screen

`Main.qml` uses a shared `now` property with a minute-aligned `Timer`, and the
root size binds to `Screen.width` / `Screen.height` instead of a fixed
1920×1080 geometry.

### Waybar dead MPRIS path removed; update check reports errors

Unused Waybar `mpris` module/CSS deleted (Costa Utils owns media).
`check_updates` distinguishes exit 0 (updates), exit 2 (up to date), and other
failures (`class: error`), and no longer duplicates the module icon in JSON
`text`.

### Runner history is private by default

History is written mode `0600`, leading-space commands are not recorded, and
`Ctrl+Shift+Delete` clears history.

### Window screenshots accept negative coordinates

Blinker window capture uses `hyprctl -j activewindow`, validates geometry, and
reports failure instead of silently falling back to a full-desktop shot.

### `deploy-user` deploys `mimeapps.list`

`scripts/deploy-user` now copies `dotfiles/mimeapps.list` into
`~/.config/mimeapps.list`, matching the installer. Overlay sync and
protocol-level Costa Utils restart remain open above.
