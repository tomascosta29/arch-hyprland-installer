The repository is not ready for a reliable fresh installation. The audit found two data-loss/boot-integrity risks, several installation-breaking integration defects, and many desktop features that cannot work with the packages currently installed.

I made no changes. The pre-existing `.gitignore` modification remains untouched.

## Critical findings

1. **`fstab` is written to the wrong location.**  
   [install.sh:94](/home/fcosta/Playground/VM/install.sh:94) writes to `/mnt/fstab` instead of `/mnt/etc/fstab`. The installed system therefore has no persistent `/boot` mount. It might boot initially, but a subsequent kernel update can write to the unmounted root filesystem’s `/boot` while GRUB continues using stale files on the EFI partition.

2. **Disk destruction safeguards are inadequate.**  
   [install.sh:27](/home/fcosta/Playground/VM/install.sh:27) accepts any block device and [install.sh:64](/home/fcosta/Playground/VM/install.sh:64) replaces its partition table. It does not reject:

   - The live boot disk.
   - Mounted disks.
   - A disk containing `/`, `/boot`, or active swap.
   - Existing LUKS/LVM/RAID members.
   - A mistyped but otherwise valid device.

   The confirmation only requires `y`, not retyping the device. The “Dual-Boot Support” title is also misleading: the selected disk is completely repartitioned.

## High-severity installer problems

3. **The user’s shell PATH is corrupted by heredoc expansion.**  
   The chroot heredoc at [install.sh:113](/home/fcosta/Playground/VM/install.sh:113) is unquoted. Consequently, `${HOME}` and `${PATH}` in [install.sh:142](/home/fcosta/Playground/VM/install.sh:142) are expanded by the live ISO’s root shell before entering the chroot—even though they visually appear inside single quotes. The installed user receives something equivalent to:

   ```bash
   export PATH="/root/.local/bin:<live-ISO-PATH>"
   ```

4. **Current Arch Hyprland compatibility is already broken and nearing complete removal.**  
   The two rules at [hyprland.conf:65](/home/fcosta/Playground/VM/dotfiles/hypr/hyprland.conf:65) fail `Hyprland --verify-config` as invalid rule syntax, even with Hyprland 0.51. Current Arch ships Hyprland 0.56, while Hyprland 0.55 introduced `hyprland.lua` and announced that legacy `hyprland.conf` support would remain for only a few releases. [Hyprland 0.55 announcement](https://hypr.land/news/update55/), [current configuration documentation](https://wiki.hypr.land/Configuring/Start/), [Arch Hyprland package](https://archlinux.org/packages/extra/x86_64/hyprland/files/).

5. **The package set does not satisfy the committed configuration.**

   | Feature | Missing or inconsistent requirement |
   |---|---|
   | File manager keybinding | `nautilus` |
   | Bluetooth menu | `bluez`, `bluez-utils`, enabled `bluetooth.service` |
   | Update checker | `pacman-contrib`, which provides `checkupdates` |
   | Media controls | `playerctl` |
   | Screen locking | `hyprlock`, `swaylock`, or `gtklock` |
   | App icons | Papirus theme requested but not installed |
   | Default theme fonts | Uses DejaVu Nerd Font, installs JetBrains Nerd Font |
   | Control center | `light`, `gammastep`, and incorrectly `makoctl` |
   | Waybar system monitor | Flatpak and `net.nokyan.Resources` |
   | Waybar click actions | Alacritty, htop, and several nonexistent scripts |
   | Clipboard persistence | `wl-paste --watch cliphist store` processes |
   | File/link opening | `xdg-open`/`xdg-utils` is not explicit |
   | Notifications | `notify-send`/`libnotify` is not explicit |

   `checkupdates` is supplied by [`pacman-contrib`](https://archlinux.org/packages/extra/x86_64/pacman-contrib/files/), and `bluetoothctl` by [`bluez-utils`](https://archlinux.org/packages/extra/x86_64/bluez-utils/files/).

6. **Clipboard history is never populated.**  
   `cliphist` is installed, but Hyprland never starts the text and image watchers. The Clipper UI will usually remain empty. Arch’s documented setup requires both `wl-paste --type text --watch cliphist store` and an image watcher. [ArchWiki clipboard configuration](https://wiki.archlinux.org/title/Hyprland#Clipboard).

7. **User creation bypasses `/etc/skel`.**  
   [install.sh:100](/home/fcosta/Playground/VM/install.sh:100) creates the home directory before [useradd -m at line 133](/home/fcosta/Playground/VM/install.sh:133). `useradd` therefore does not populate the skeleton files. The installer later creates a minimal `.bashrc` containing the broken PATH but may leave out `.bash_profile` and normal Arch defaults.

8. **Failure cleanup and preflight checks are absent.**

   - No check for UEFI mode.
   - No network or repository check.
   - No minimum disk-size check.
   - No partition-node verification.
   - `udevadm settle` runs only if `partprobe` fails.
   - No trap to unmount `/mnt` after an error.
   - No detection of an already-mounted `/mnt`.
   - Rerunning appends duplicate GRUB and environment settings.
   - Theme initialization failure is discarded with `|| true`.

9. **Security and account setup need hardening.**

   - The user and root receive the same password.
   - Empty passwords are not rejected.
   - Password entry has no confirmation.
   - Usernames and hostnames are not validated.
   - A username beginning with `-` can be interpreted as a `useradd` option.
   - Root could be locked instead of sharing the user password.
   - No CPU microcode package is installed.

## Broken desktop integrations

10. **Waybar still contains Fedora-host configuration.**  
    [modules:224](/home/fcosta/Playground/VM/dotfiles/waybar/modules:224) labels updates as Fedora/dnf, and [line 230](/home/fcosta/Playground/VM/dotfiles/waybar/modules:230) runs `sudo dnf upgrade` through a nonexistent Alacritty wrapper. This cannot work on the installed Arch guest.

11. **Multiple Waybar actions reference absent software or files.**

    - Resources/Flatpak: [modules:111](/home/fcosta/Playground/VM/dotfiles/waybar/modules:111)
    - Alacritty and htop: [modules:121](/home/fcosta/Playground/VM/dotfiles/waybar/modules:121)
    - Missing `rofi_disk`: [modules:163](/home/fcosta/Playground/VM/dotfiles/waybar/modules:163)
    - Missing `rofi_sensors`: [modules:170](/home/fcosta/Playground/VM/dotfiles/waybar/modules:170)

12. **Hardware telemetry is not portable.**

    - GPU load assumes AMD sysfs paths and only cards 0/1: [modules:125](/home/fcosta/Playground/VM/dotfiles/waybar/modules:125)
    - VRAM assumes `card1` and has no fallback: [modules:133](/home/fcosta/Playground/VM/dotfiles/waybar/modules:133)
    - Temperature assumes `hwmon5`: [modules:167](/home/fcosta/Playground/VM/dotfiles/waybar/modules:167)

    These are particularly inappropriate for a VM-targeted default.

13. **Two themes omit most colors required by Waybar.**  
    [style.css](/home/fcosta/Playground/VM/dotfiles/waybar/style.css:11) uses `background-alt1` through `background-alt4`, `foreground-dim`, and numerous `soft-*` names. The [Catppuccin palette](/home/fcosta/Playground/VM/dotfiles/themes/catppuccin-macchiato/colors.css:1) and [Tokyo Night palette](/home/fcosta/Playground/VM/dotfiles/themes/tokyo-night/colors.css:1) do not define most of them. Switching from `fcosta` can leave large portions of the Waybar CSS invalid.

14. **Dunst configuration uses removed/deprecated geometry syntax.**  
    All three themes use `geometry = "300x5-30+50"`, for example [fcosta/dunstrc:9](/home/fcosta/Playground/VM/dotfiles/themes/fcosta/dunstrc:9). Current Dunst replaces this with `width`, `height`, `origin`, `offset`, and `notification_limit`. [Current Dunst documentation](https://dunst-project.org/documentation/).

15. **Theme switching is non-atomic and overstates live application.**

    - An interruption can leave a partially changed desktop.
    - No theme schema/completeness validation occurs.
    - It kills every user Waybar, Dunst, and Hyprpaper process by name.
    - Kitty is not actually live-reloaded.
    - GTK4 CSS is copied verbatim into GTK3.
    - It reports success even if a theme lacks components.
    - Runtime-generated files are mixed with source configuration.

16. **Monitor switching accepts typos as “single” mode.**  
    Any argument other than `dual` or `dual-host` silently selects single mode at [monitor-select:25](/home/fcosta/Playground/VM/dotfiles/scripts/monitor-select:25). The file is overwritten non-atomically and reloaded without validation or rollback.

## `costa-utils` defects

17. **Bluetooth refresh crashes immediately.**  
    [bluetooth_menu.py:206](/home/fcosta/Playground/VM/dotfiles/costa-utils/costautils/bluetooth_menu.py:206) calls `subprocess.run`, but `subprocess` is never imported. Ruff reports three `F821` undefined-name errors.

18. **Bluetooth refresh enables discoverability instead of scanning.**  
    The same line runs `bluetoothctl discoverable on`, permanently making the computer visible to nearby devices. It does not discover devices and never turns discoverability off.

19. **The screenshot manager watches the wrong directory on a fresh installation.**  
    At import time, [blinker_manager.py:25](/home/fcosta/Playground/VM/dotfiles/costa-utils/costautils/blinker_manager.py:25) falls back from `~/Pictures/Screenshots` to `~/Pictures` if the screenshot directory does not yet exist. The capture utility later creates `~/Pictures/Screenshots`, but the already-running manager continues watching and listing `~/Pictures`. It also completely ignores the configurable `screenshot_dir`.

20. **Unified application mode omits module-specific actions.**  
    `CostaUtilsApp` registers only `activate-target` at [costa_utils.py:60](/home/fcosta/Playground/VM/dotfiles/costa-utils/costa_utils.py:60). Therefore:

    - Blinker Manager’s `app.settings` and `app.about` menu items do not work.
    - Clipper’s `app.about` does not work.
    - Standalone dependency checks and startup behavior are skipped.

21. **Blinker Manager grid mode is largely nonfunctional.**  
    Grid selection updates `current_file`, but copy, pin, move, and delete still read selected rows from the list view, for example [blinker_manager.py:809](/home/fcosta/Playground/VM/dotfiles/costa-utils/costautils/blinker_manager.py:809). Those actions become no-ops in grid mode.

22. **Blinker Manager keyboard navigation calls a removed GTK3 API.**  
    [blinker_manager.py:958](/home/fcosta/Playground/VM/dotfiles/costa-utils/costautils/blinker_manager.py:958) calls `Gtk.ListBox.get_children()`. That API does not exist in GTK4; this was also confirmed against the host’s GTK 4.20.

23. **Screenshot operations can overwrite or lose data.**

    - Filenames have only one-second precision: [blinker.py:291](/home/fcosta/Playground/VM/dotfiles/costa-utils/costautils/blinker.py:291)
    - Two captures in the same second can overwrite.
    - Moving to a directory with an existing filename has no collision check: [blinker_manager.py:893](/home/fcosta/Playground/VM/dotfiles/costa-utils/costautils/blinker_manager.py:893)
    - Moving or deleting pinned screenshots leaves stale pin paths.
    - Capture success calls `application.quit()`, terminating the entire unified utility daemon and any other open utility windows.

24. **Clipper’s first close can fail.**  
    [cliphist_gtk.py:59](/home/fcosta/Playground/VM/dotfiles/costa-utils/costautils/cliphist_gtk.py:59) writes `~/.config/clipper/state.json` without creating its directory first.

25. **“Pinned items will be preserved” is false.**  
    The dialog says pins survive, but [cliphist_gtk.py:562](/home/fcosta/Playground/VM/dotfiles/costa-utils/costautils/cliphist_gtk.py:562) runs `cliphist wipe`, deleting every underlying entry. Only stale numeric IDs remain in the pins file.

26. **Clipper image copies lose MIME information.**  
    [cliphist_gtk.py:602](/home/fcosta/Playground/VM/dotfiles/costa-utils/costautils/cliphist_gtk.py:602) sends decoded bytes to `wl-copy` without specifying the image MIME type. Applications may receive the screenshot as generic binary or text.

27. **Power actions are dangerously immediate.**

    - Reboot, shutdown, logout, suspend, and hibernate have no confirmation.
    - Logout uses `loginctl terminate-user`, terminating every session belonging to the user, not merely Hyprland.
    - Hibernate is exposed despite no swap being configured.
    - “Lock” falls back to a loginctl signal even though no locker is installed.

28. **Network handling fails for legitimate SSIDs and passwords.**

    - Terse `nmcli` output is naïvely split on `:`, breaking escaped colons in SSIDs: [network_menu.py:197](/home/fcosta/Playground/VM/dotfiles/costa-utils/costautils/network_menu.py:197)
    - Saved connection profile names are assumed to equal SSIDs.
    - Open and enterprise networks are treated like WPA-password networks.
    - `.strip()` changes valid passwords with leading/trailing spaces.
    - SSIDs are inserted into GTK markup without escaping at [network_menu.py:292](/home/fcosta/Playground/VM/dotfiles/costa-utils/costautils/network_menu.py:292).
    - The Wi-Fi password is passed in `nmcli`’s process arguments.

29. **Control Center has several dead controls.**

    - `self.adapter_path` is used but never initialized: [control_center.py:394](/home/fcosta/Playground/VM/dotfiles/costa-utils/costautils/control_center.py:394)
    - DND controls `makoctl` while the desktop runs Dunst.
    - Brightness expects `light`.
    - Night light expects `gammastep`.
    - It is not exposed through the committed Waybar or Hyprland UI.

30. **Volume and media monitoring stops permanently after first hide.**  
    `VolumeWindow` and `ControlCenterWindow` stop `playerctl` monitoring on focus loss but never restart it when the singleton presents the existing window again.

31. **Several subprocesses block the GTK main loop.**  
    Volume sliders, brightness sliders, media buttons, Clipper decoding, thumbnail generation, file moves, and other operations run synchronously. Slow D-Bus, PipeWire, filesystem, or command activity can freeze every utility because they all share one singleton process.

32. **Remote artwork downloads are unbounded.**  
    Volume and Control Center read complete HTTP responses into memory. A malicious or broken MPRIS player can point artwork metadata to a huge response and exhaust the daemon’s memory.

33. **The singleton can silently lose commands.**  
    D-Bus forwarding has a 500 ms timeout. If the shared GTK process is blocked, the launcher may fail forwarding, detect a remote application, retry once, ignore that result, and still return success.

## Repository and maintenance problems

34. Python 3.14 `__pycache__` and `.pyc` files are committed. They are interpreter-specific, unnecessary, and account for roughly 288 KiB.

35. `.gitignore` does not currently ignore `__pycache__/` or `*.py[cod]`.

36. There are no tests, CI workflows, packaging metadata, dependency manifest, desktop files, service files, license, or release process.

37. Ruff reports **317 issues**. Most are formatting/style debt, but the Bluetooth undefined-name failure is a real runtime defect. Ten of thirteen Python files also fail `ruff format --check`.

38. `config` and `config.jsonc` are identical duplicate Waybar configurations, creating future drift risk.

39. The `fcosta` and Catppuccin wallpapers are the same Git blob, while the Tokyo Night file is JPEG data stored with a `.png` extension.

40. Wallpaper provenance and redistribution licensing are undocumented.

41. [uninstall.sh](/home/fcosta/Playground/VM/dotfiles/costa-utils/uninstall.sh:1) does not reverse the installer:

    - It leaves `/usr/local/bin/costa-utils`.
    - It leaves the duplicated `~/.config/costa-utils` source tree.
    - It removes aliases and desktop entries the installer never created.
    - It leaves generated themes and state.
    - It has no strict mode or ownership checks.

42. The README documents a specific Fedora host rather than this Arch installer. It lacks installation steps, destructive-operation warnings, supported hardware, dependency information, VM setup for `archlinux`, troubleshooting for the actual desktop, and recovery instructions.

## Validation performed

- All Bash files passed `bash -n`.
- All 13 Python files parsed successfully as Python syntax.
- Waybar JSONC files parsed after comment removal.
- All eight SVGs passed XML validation.
- Hyprland parser validation confirmed both window rules are invalid.
- Ruff found 317 issues, including the Bluetooth runtime failure.
- No obvious secrets were found in tracked text.
- No test suite or CI configuration exists.
- ShellCheck was not installed, so Bash semantic linting was not available.
- I did not deploy, alter the VM, commit, or push anything.

## Recommended repair order

1. Fix `fstab`, quote the chroot heredoc, harden disk selection, add cleanup traps and preflight checks.
2. Build one explicit package/dependency manifest and make every configured action depend only on it.
3. Migrate to `hyprland.lua` for current Arch, including Lua-based monitor/theme integration.
4. Add clipboard watchers, a real locker, Bluetooth packages/service, and Arch-correct Waybar actions.
5. Repair the unified `costa-utils` action registration and screenshot-directory model.
6. Fix destructive operations, confirmations, grid mode, GTK4 APIs, and subprocess threading.
7. Add Ruff/formatting, ShellCheck, JSONC/CSS/config validators, and pytest coverage.
8. Add a disposable QEMU installation test that verifies partitions, `/etc/fstab`, boot, services, Hyprland config, Waybar startup, and every `costa-utils` launcher.
9. Replace the README with an actual installation/recovery guide and document asset licensing.
