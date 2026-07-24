# Technical polish review

Reviewed: 2026-07-24

This review deliberately excludes repository cosmetics, licensing, screenshots,
badges, changelogs, and similar maintenance. It focuses on the behavior and
architecture of the installer, session configuration, switchers, Waybar, SDDM,
and Costa Utils.

## Executive opinion

The repository has moved past basic repair. The installer has sensible
destructive-operation safeguards, the desktop stack is coherent, and the Lua
migration is substantially correct. The remaining polish is not another round
of formatting or small guards. It is about owning process lifecycle, removing
duplicated sources of truth, and giving Costa Utils a real backend architecture.

The three highest-leverage changes are:

1. manage the Hyprland session through systemd user targets and units;
2. make Lua the only Hyprland configuration source; and
3. separate Costa Utils' system backends from its GTK windows, with bounded and
   cancellable background work.

Those changes would address most of the current reload inconsistencies,
silent failures, races, duplicated code, and difficult-to-test behavior.

## Priority 1: own the desktop session lifecycle

### The session is a collection of unsupervised child processes

[`dotfiles/hypr/hyprland.lua`](dotfiles/hypr/hyprland.lua) directly starts
Hyprpaper, Hypridle, Hyprsunset, Waybar, Dunst, Spice vdagent, and both clipboard
watchers from the `hyprland.start` event. Only the policy agent is started as a
systemd user service.

This works when every process starts correctly and remains alive. It has weak
behavior when anything crashes:

- there is no restart policy;
- startup failures are not surfaced;
- there is no declared ordering after the Wayland and D-Bus environment exists;
- theme switching has to kill and recreate Hyprpaper itself;
- there is no matching `hyprland.shutdown` cleanup;
- the graphical session target is never started, so other user services and
  portals do not have a proper session lifetime to bind to.

The right end state is a `hyprland-session.target` bound to
`graphical-session.target`, plus user units for the bar, notification daemon,
wallpaper, idle daemon, night-light daemon, and clipboard watchers. Hyprland
should import its environment and start the target once. Theme switching should
reload or restart those units instead of managing arbitrary processes.

This is also the right place to decide whether GNOME Keyring is real or dead
weight. The installer includes `gnome-keyring`, but it does not configure PAM
unlocking or explicitly start the secrets component. Either integrate it into
the login/session lifecycle or remove it.

Hyprland's current documentation recommends this target-based approach for
services that belong to the graphical session:
<https://wiki.hypr.land/Useful-Utilities/Systemd-start/>.

### The exit path should use the compositor's supported shutdown mechanism

The Lua configuration binds `SUPER+SHIFT+M` to `hl.dsp.exit()`. Current
Hyprland documentation recommends `hyprshutdown` instead of the raw exit
dispatcher. A managed session target makes this especially important because
shutdown should stop the session graph cleanly rather than merely terminate the
compositor.

## Priority 2: stop maintaining two Hyprland implementations

The repository declares Hyprland 0.55 or newer as supported, yet every major
configuration mutation still maintains both Lua and legacy Hyprlang:

- `hyprland.lua` and `hyprland.conf`;
- `current_colors.lua` and `current_colors.conf`;
- `input.lua` and `input.conf`;
- `monitors.lua` and `monitors.conf`;
- Lua and Hyprlang copies of every monitor profile;
- dual writes in `theme-select`, `monitor-select`, and `desktop-settings`.

They are already semantically different. The Lua dual-monitor profile enables
VRR and persistent workspaces; the legacy profile does not. The Lua config sets
cursor sizes and compositor miscellaneous options that the legacy config omits.
Every future keybinding or rule change has to be translated manually.

If 0.55+ is the real support boundary, remove the legacy path. If an old VM
must remain supported, generate the legacy files from one declarative data
source instead of editing both by hand. Keeping two handwritten implementations
is the largest source of avoidable configuration drift in the repository.

## Priority 3: give Costa Utils a backend and job model

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

The installer deploys `mimeapps.list`; `scripts/deploy-user` does not. The
deployer also overlays directories with `cp -a`, leaving files that were removed
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

The package/configuration phase also installs `os-prober` and enables
`GRUB_DISABLE_OS_PROBER=false` even though the installer explicitly destroys the
selected disk and declares no dual-boot support. Remove that integration unless
detecting operating systems on other attached disks is an intentional feature.

Finally, add preflight checks for repository/network reachability and clock
health before the partition table is replaced. A `pacstrap` failure is safe in
the data-integrity sense, but discovering network or time problems after wiping
the disk is poor installation behavior.

### The SDDM clock is static

[`dotfiles/sddm/costa/Main.qml`](dotfiles/sddm/costa/Main.qml) binds its time and
date text directly to `new Date()` but has no `Timer` or changing property. QML
evaluates those bindings at creation, so the greeter clock does not advance.
Use a minute-aligned timer that updates a shared date property.

The fixed 1920×1080 root declaration should also be tested on the VM's dynamic
SPICE resolution and on both 1440p monitors. The child layout is anchored well,
but the root should take its size from the actual greeter view rather than
expressing one preferred screen as the theme's geometry.

### Waybar has configuration that disagrees with its visible bar

The MPRIS module and its CSS are defined but the module is not included in any
module list. Media controls instead live inside Costa Utils. Pick one owner and
delete the inactive implementation.

The update helper emits an icon in its JSON `text`, while the Waybar custom
module adds another icon around that text. More importantly,
`check_updates` converts missing tools, network errors, mirror failures, and
“no updates” into the same zero-update result. “System up to date” should only
be displayed for the expected no-update exit status; failures need an error
class and tooltip.

The static usage drawer also exposes AMD GPU and VRAM modules in the VM, where
they intentionally return zero. A machine/profile-aware module list would be
cleaner than showing plausible-looking zero telemetry for nonexistent hardware.

Waybar workspace clicking should be explicitly tested with the supported
Hyprland Lua/Waybar combination. There is current upstream compatibility work
around the workspace module dispatching old-style commands into Lua-based
Hyprland:
<https://github.com/Alexays/Waybar/issues/5008>.

### Runner history can retain secrets

The Runner stores the last 50 raw shell commands in
`~/.local/state/costa-utils/runner_history.json`. Commands commonly contain API
tokens, URLs with credentials, or one-off passwords. The file is created using
the process umask rather than an explicit private mode, and there is no
history-disable or clear-history control.

The shell execution itself is intentional for a runner. The polish needed is
around secret retention: create the file as `0600`, allow private commands not
to be recorded, and provide a clear-history action.

### Screenshot window capture has multi-monitor edge cases

Window capture parses the text form of `hyprctl activewindow` with expressions
that only accept non-negative coordinates. A window on a monitor positioned to
the left or above the origin can have negative coordinates; parsing then fails
and silently falls back to a full-desktop screenshot.

Use `hyprctl -j activewindow`, validate the geometry, and report failure instead
of changing capture mode. Normalize the configured screenshot directory once in
a shared backend so the launcher and manager cannot interpret relative paths
differently.

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

1. Introduce the graphical-session target and supervised user services.
2. Remove the legacy Hyprland path and simplify all switchers to Lua only.
3. Extract shared audio, MPRIS, NetworkManager, BlueZ, and command-runner
   backends.
4. Add bounded jobs, cancellation/generations, logging, and private state-file
   handling.
5. Fix Bluetooth onboarding, Clipper caching/decoding, night-light state, and
   SDDM time updates.
6. Make theme and user deployment manifest-driven and transaction-oriented.
7. Harden installer partition discovery and pre-wipe preflight checks.
8. Add the installed-VM smoke test once the new service graph is stable.
