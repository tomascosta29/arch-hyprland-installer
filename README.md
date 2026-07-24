# Arch Hyprland Installer

An opinionated Arch Linux workstation installer for AMD systems and QEMU/KVM
guests. It installs Hyprland, a coherent Wayland desktop, themed dotfiles, and
the GTK4 `costa-utils` suite.

> [!CAUTION]
> `install.sh` is destructive. It replaces the partition table on the selected
> whole disk and erases every partition on that disk. It does not provide
> dual-boot, encryption, a separate home partition, or swap. Back up your data
> and verify the target device carefully.

## Supported setup

- UEFI x86-64 system using GPT and GRUB.
- AMD CPU and AMD GPU as the primary hardware target.
- QEMU/KVM guest using VirtIO graphics and the QEMU guest agent.
- Current Arch packages and Hyprland 0.55 or newer.
- One normal user with sudo access; direct root login remains locked.

The AMD baseline includes `amd-ucode`, Mesa, RADV (`vulkan-radeon`), VA-API,
and their diagnostic tools. Current Mesa provides the AMD VA-API driver
directly. These packages also coexist with the Mesa VirtIO path used by the VM.

## Deliberate desktop choices

There is one default tool for each core job:

| Job | Choice |
|---|---|
| Compositor | Hyprland |
| Terminal | Kitty |
| File manager | Nautilus |
| Application launcher | Costa Utils |
| Lock and idle | Hyprlock + Hypridle |
| Notifications | Dunst |
| Wallpaper | Hyprpaper |
| Night light | Hyprsunset |
| Policy agent | Hyprpolkitagent |
| Network | NetworkManager |
| Bluetooth | BlueZ |
| Audio | PipeWire + WirePlumber |
| Package updates | Pacman + `checkupdates` |
| System monitor | htop in Kitty |

## Installation

Boot the latest Arch installation ISO in UEFI mode, connect it to the network,
and obtain this repository:

```bash
pacman -Sy --needed git
git clone https://github.com/tomascosta29/arch-hyprland-installer.git
cd arch-hyprland-installer
chmod +x install.sh
./install.sh
```

The installer:

1. validates UEFI mode, input values, target capacity, mounts, and active swap;
2. requires the full canonical disk path as destructive confirmation;
3. creates a 1 GiB EFI partition and an ext4 root partition;
4. writes the generated filesystem table to `/etc/fstab`;
5. installs and configures the AMD-first desktop package set;
6. creates the user before deploying dotfiles;
7. enables NetworkManager, Bluetooth, and SDDM (VM agents use their packaged
   udev/session activation);
8. applies the default Nordfox theme; and
9. unmounts the target filesystems on success or failure.

Remove the ISO and reboot after the success message.

## VM profile

The expected libvirt guest is named `archlinux`, uses `qemu:///system`, UEFI,
8 GiB RAM, four host-passthrough vCPUs, and a disk of at least 40 GiB. Attach a
VirtIO graphics device plus a SPICE display if desktop integration is wanted.

The installed `qemu-guest-agent` and `spice-vdagent` packages allow host-side
inspection and clipboard/display integration. On bare metal, their absence of a
VirtIO/SPICE channel is harmless.

## Desktop controls

| Shortcut | Action |
|---|---|
| `Super+Return` or `Super+Q` | Kitty |
| `Super+E` | Nautilus |
| `Super+R` | Application menu |
| `Super+V` | Clipboard history |
| `Super+P` | Confirmed power menu |
| `Super+L` | Lock session |
| `Print` | Screenshot launcher |
| `Super+Alt+T` | Theme selector |
| `Super+Alt+M` | Monitor selector |
| `Super+Shift+M` | Exit Hyprland |

Clipboard text and image watchers start with Hyprland. The power menu omits
hibernate because the installer intentionally creates no swap.

## Themes and monitors

```bash
~/.config/scripts/theme-select fcosta
~/.config/scripts/theme-select catppuccin-macchiato
~/.config/scripts/theme-select tokyo-night

~/.config/scripts/monitor-select single
~/.config/scripts/monitor-select dual
```

Switching validates all required theme files and atomically replaces individual
runtime files. Both current Lua and legacy Hyprland monitor/color files are kept
in sync. The dual profile targets:

- `DP-1`: 2560×1440 at 180 Hz with adaptive sync;
- `HDMI-A-1`: 2560×1440 at 144 Hz.

Use `single` in a VM or when connector names do not match.

## Validation

Run the repository checks before installation:

```bash
./scripts/validate
```

The suite checks Bash syntax, Python formatting and lint, unit tests, Lua syntax,
desktop entries, and repository invariants. It currently covers installer
safety properties, AMD dependencies, theme completeness, SSID escaping,
clipboard MIME handling, launcher dispatch, and screenshot collision handling.

To update an existing user's checked-out configuration without running the
destructive installer:

```bash
./scripts/deploy-user fcosta
```

The deployer copies the supported config components, installs Costa Utils and
its desktop metadata, applies the selected theme, and restarts any running Costa
Utils process. It does not alter system packages or partitions.

## Recovery

If installation stops, the exit trap attempts to unmount `/mnt`. Verify before
retrying:

```bash
findmnt /mnt
lsblk -f
```

To repair an installed system from the ISO:

```bash
mount /dev/disk/by-label/arch-root /mnt
mount /dev/disk/by-label/EFI /mnt/boot
arch-chroot /mnt
```

Inside the chroot, useful checks are:

```bash
cat /etc/fstab
grub-mkconfig -o /boot/grub/grub.cfg
systemctl enable NetworkManager bluetooth sddm
```

From a running Hyprland session:

```bash
hyprctl configerrors
systemctl --user status hyprpolkitagent
journalctl --user -b --no-pager
```

## Repository layout

- `install.sh` — destructive Arch installation and system configuration.
- `dotfiles/hypr` — current Lua config plus legacy VM compatibility config.
- `dotfiles/themes` — complete theme bundles.
- `dotfiles/waybar` — bar config, modules, and style.
- `dotfiles/costa-utils` — GTK4 desktop utility suite.
- `tests` — unit and repository-invariant tests.
- `ISSUES.md` — the original exhaustive audit that drove the repair order.

Wallpaper provenance is recorded in [ASSETS.md](ASSETS.md). Code licensing still
needs an explicit decision from the repository owner before public redistribution.
