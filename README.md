# Arch Hyprland Installer

An opinionated Arch Linux workstation installer for AMD systems and QEMU/KVM
guests. It installs Hyprland, a coherent Wayland desktop, themed dotfiles, and
the Rust GTK4 `costa-utils` suite.

> [!CAUTION]
> `install.sh` is destructive. It replaces the partition table on the selected
> whole disk and erases every partition on that disk. A Windows installation on
> a different disk is supported and left untouched, but same-disk dual boot,
> encryption, a separate home partition, and swap are not provided. Back up
> your data and verify the target device carefully.

## Supported setup

- UEFI x86-64 system using GPT and GRUB.
- AMD CPU and AMD GPU as the primary hardware target.
- QEMU/KVM guest using VirtIO graphics and the QEMU guest agent.
- Dual-disk Windows installations discovered by GRUB through `os-prober`.
- Current Arch packages and Hyprland 0.55 or newer.
- One normal user with sudo access; direct root login remains locked.

The shared desktop baseline includes Mesa and graphics diagnostic tools. The
bare-metal profile adds `amd-ucode`, RADV (`vulkan-radeon`), firmware updates,
NVMe/SMART diagnostics, and NTFS support for the separate Windows disk. The VM
profile adds only the QEMU guest and SPICE agents.

## Deliberate desktop choices

There is one default tool for each core job:

| Job | Choice |
|---|---|
| Compositor | Hyprland |
| Terminal | Kitty |
| Browser | Firefox |
| File manager | Nautilus |
| Application launcher | Costa Utils |
| Lock and idle | Hyprlock + Hypridle |
| Display manager | SDDM (`costa` theme) |
| Notifications | Dunst |
| Wallpaper | Hyprpaper |
| Night light | Hyprsunset |
| Policy agent | Hyprpolkitagent |
| Network | NetworkManager |
| Bluetooth | BlueZ |
| Audio | PipeWire + WirePlumber |
| Package updates | Pacman + `checkupdates` |
| System monitor | htop in Kitty |

## Release Flavors

The installer supports two distinct release flavors:

| Flavor | Default Shell | Neovim Setup | Profile Characteristics |
|---|---|---|---|
| **Full** (default) | `zsh` with Oh My Zsh and plugins | Neovim with LazyVim starter pre-installed | Full-featured, opinionated configuration |
| **Light** | `bash` | Stock Neovim, no pre-installed configs | Minimal, less opinionated, unbloated |

During the interactive installation, you can confirm or select the desired flavor. Pre-packaged GitHub release archives (e.g. `arch-hyprland-installer-light.tar.gz`) default to their respective flavors and automatically exclude unneeded configuration assets.

To package these releases locally:
```bash
./scripts/build-release.sh
```

## Installation

Boot the latest Arch installation ISO in UEFI mode, connect it to the network, and run the single-command bootstrap installer:

```bash
curl -s https://arch.tomascosta.pt | bash
```

Alternatively, if you prefer cloning the repository manually:

```bash
pacman -Sy --needed git
git clone https://github.com/tomascosta29/arch-hyprland-installer.git
cd arch-hyprland-installer
chmod +x install.sh
./install.sh
```

The installer:

1. validates UEFI mode, input values, target capacity, mounts, active swap,
   clock health, and Arch mirror reachability before asking for a password or
   destructive confirmation;
2. selects a detected `bare-metal` or `vm` package profile;
3. requires the full canonical disk path as destructive confirmation;
4. leaves every non-target disk untouched, allowing a separate Windows disk;
5. creates a 1 GiB EFI partition and an ext4 root partition, then discovers and
   verifies both through `lsblk` instead of guessing device suffixes;
6. writes the generated filesystem table to `/etc/fstab`;
7. installs the shared desktop packages and the selected hardware profile;
8. creates the user before deploying dotfiles;
9. enables NetworkManager, firewalld, Bluetooth, and SDDM;
10. configures SDDM login unlocking and password synchronization for GNOME
    Keyring;
11. applies the default Nordfox theme; and
12. unmounts the target filesystems on success or failure.

Remove the ISO and reboot after the success message.

For a two-drive Windows setup, select only the drive dedicated to Arch. The
installer destroys that selected drive, never repartitions the Windows drive,
and includes the Windows boot entry when `os-prober` can discover it.

## VM profile

The expected libvirt guest is named `archlinux`, uses `qemu:///system`, UEFI,
8 GiB RAM, four host-passthrough vCPUs, and a disk of at least 40 GiB. It uses
3D-accelerated VirtIO graphics via `egl-headless` (host GPU virgl) plus a local
SPICE console without SPICE-GL. Hyprland needs the 3D path; skipping SPICE-GL
avoids the common mosaic/tiled console corruption on AMD hosts.

Create that VM and attach an Arch ISO with:

```bash
./scripts/create-vm ~/Downloads/archlinux-x86_64.iso
```

The script disables Secure Boot, starts the VM, and leaves all installation,
testing, and cleanup to you.

The VM profile's `qemu-guest-agent` and `spice-vdagent` packages allow host-side
inspection and clipboard/display integration. They are not installed by the
bare-metal profile.

## Desktop controls

| Shortcut | Action |
|---|---|
| `Super+Return` or `Super+Q` | Kitty |
| `Super+E` | Nautilus |
| `Super+B` | Firefox |
| `Super+R` | Application menu |
| `Super+V` | Clipboard history |
| `Super+P` | Confirmed power menu |
| `Super+L` | Lock session |
| `Print` | Screenshot launcher |
| `Super+Alt+T` | Theme selector |
| `Super+Alt+M` | Monitor selector |
| `Super+Alt+K` | Keyboard and clock settings |
| `Super+Shift+M` | Exit Hyprland |

Hyprland starts a user `hyprland-session.target`. Systemd then supervises
Hyprpaper, Hypridle, Hyprsunset, Quickshell, Dunst, the policy agent, and both
clipboard watchers with restart-on-failure behavior and stops them with the
compositor. The power menu omits hibernate because the installer intentionally
creates no swap. The Costa launcher hides duplicate utilities and non-Firefox
browsers so each role stays singular.

## Themes and monitors

```bash
~/.config/scripts/theme-select fcosta
~/.config/scripts/theme-select catppuccin-macchiato
~/.config/scripts/theme-select tokyo-night

~/.config/scripts/monitor-select single
~/.config/scripts/monitor-select dual

~/.config/scripts/desktop-settings --keyboard pt --clock 24h
~/.config/scripts/desktop-settings --show
```

Switching validates the color schema and Lua before changing state, then moves
every runtime consumer with one atomic `current-theme` symlink. Supervised
services reload, Kitty receives `SIGUSR1`, and Costa Utils exits cleanly so its
next invocation uses the new GTK theme. SDDM is deliberately installation-time
only because a desktop user should not mutate `/usr/share`. Hyprland 0.55's Lua
format is the only compositor configuration source. The dual profile targets:

- `DP-1`: 2560×1440 at 180 Hz with adaptive sync;
- `HDMI-A-1`: 2560×1440 at 144 Hz.

Use `single` in a VM or when connector names do not match.

## Validation

Run the repository checks before installation:

```bash
./scripts/validate
```

The suite checks Bash syntax, Python formatting and lint, unit tests, Lua syntax,
desktop entries, and repository invariants. It covers installer safety and
profile properties, theme completeness/transactions, shared backend parsing,
bounded job delivery, clipboard MIME handling, deployment ownership, launcher
dispatch, and screenshot collision handling.

To update an existing user's checked-out configuration without running the
destructive installer:

```bash
./scripts/deploy-user fcosta
```

The deployer synchronizes an exact ownership manifest while preserving unowned
user files, installs the user service graph and Costa Utils metadata, chooses a
VM or bare-metal Quickshell profile, applies the selected theme, and shuts down an
old Costa Utils singleton through its application protocol. A currently running
older session is migrated to the supervised target without waiting for logout.
It does not alter system packages or partitions. For isolated staging tests,
`COSTA_DEPLOY_RELOAD=0` suppresses every live session reload.

After installing and logging into the VM, validate the deployed machine from
the host through QEMU Guest Agent:

```bash
./scripts/vm-smoke archlinux fcosta
```

The same checks can be run inside an installed machine with
`~/.config/scripts/validate-installed`.

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
systemctl enable NetworkManager firewalld bluetooth sddm
```

From a running Hyprland session:

```bash
hyprctl configerrors
systemctl --user status hyprland-session.target
systemctl --user --failed
journalctl --user -b --no-pager
```

## Repository layout

- `install.sh` — destructive Arch installation and system configuration.
- `dotfiles/hypr` — Hyprland 0.55+ Lua configuration and component configs.
- `dotfiles/systemd/user` — supervised graphical-session target and user units.
- `dotfiles/themes` — complete theme bundles.
- `dotfiles/quickshell` — Quickshell `costa` bar (replaces Waybar).
- `costa-utils` — Rust GTK4/libadwaita desktop utility suite (overlays + CLI).
- `tests` — unit and repository-invariant tests.
- `INTERFACES.md` — stable contracts between desktop components (theme packs,
  Quickshell, session, costa-utils CLI, deploy layout).
- `POLISH.md` — the expert runtime/architecture review and implementation record.

Wallpaper provenance is recorded in [ASSETS.md](ASSETS.md). Code licensing still
needs an explicit decision from the repository owner before public redistribution.
