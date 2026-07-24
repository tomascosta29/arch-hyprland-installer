#!/usr/bin/env bash
# Arch Linux + Hyprland workstation installer (UEFI/GPT, destructive).

set -Eeuo pipefail

readonly GREEN='\033[0;32m'
readonly CYAN='\033[0;36m'
readonly YELLOW='\033[1;33m'
readonly RED='\033[0;31m'
readonly NC='\033[0m'
readonly MIN_DISK_BYTES=$((20 * 1024 * 1024 * 1024))

TARGET_MOUNTED=false

log() {
    printf '%b%s%b\n' "${GREEN}" "$1" "${NC}"
}

warn() {
    printf '%bWarning: %s%b\n' "${YELLOW}" "$1" "${NC}" >&2
}

die() {
    printf '%bError: %s%b\n' "${RED}" "$1" "${NC}" >&2
    exit 1
}

cleanup() {
    local exit_status=$?

    if [[ "${TARGET_MOUNTED}" == true ]] && mountpoint -q /mnt; then
        log "Unmounting the target filesystems..."
        umount -R /mnt || warn "Could not fully unmount /mnt; unmount it before rebooting."
    fi

    if ((exit_status != 0)); then
        printf '%bInstallation stopped with an error (exit %d).%b\n' \
            "${RED}" "${exit_status}" "${NC}" >&2
    fi
}

trap cleanup EXIT

require_commands() {
    local command_name

    for command_name in \
        arch-chroot findmnt genfstab lsblk mkfs.ext4 mkfs.fat mount \
        mountpoint pacstrap partprobe readlink sfdisk swapon udevadm umount; do
        command -v "${command_name}" >/dev/null 2>&1 ||
            die "Required command '${command_name}' is unavailable. Boot the current Arch ISO."
    done
}

validate_target_disk() {
    local disk_size disk_type child_device

    [[ -b "${DISK}" ]] || die "Target '${DISK}' is not a block device."
    DISK="$(readlink -f "${DISK}")"
    disk_type="$(lsblk -dnro TYPE "${DISK}")"
    [[ "${disk_type}" == "disk" ]] ||
        die "Target '${DISK}' is a '${disk_type}', not a whole disk."

    disk_size="$(lsblk -bdnro SIZE "${DISK}")"
    [[ "${disk_size}" =~ ^[0-9]+$ ]] ||
        die "Could not determine the capacity of '${DISK}'."
    ((disk_size >= MIN_DISK_BYTES)) ||
        die "Target disk must be at least 20 GiB."

    while IFS= read -r child_device; do
        [[ -n "${child_device}" ]] || continue
        if findmnt -rn -S "${child_device}" >/dev/null 2>&1; then
            die "'${child_device}' is mounted. Unmount every target-disk filesystem first."
        fi
        if swapon --noheadings --raw --show=NAME | grep -Fxq "${child_device}"; then
            die "'${child_device}' is active swap. Disable it before installing."
        fi
    done < <(lsblk -lnpo NAME "${DISK}")
}

wait_for_partition() {
    local partition=$1 attempt

    for attempt in {1..20}; do
        [[ -b "${partition}" ]] && return 0
        udevadm settle
        sleep 0.25
    done

    die "Partition '${partition}' did not appear after partitioning."
}

printf '%b\n' "${CYAN}"
printf '%s\n' \
    "========================================================================" \
    "         Arch Linux + Hyprland Workstation Installer" \
    "========================================================================"
printf '%b\n' "${NC}"

((EUID == 0)) || die "Run this installer as root from the Arch Linux live ISO."
[[ -d /sys/firmware/efi/efivars ]] ||
    die "The live ISO is not booted in UEFI mode."
require_commands

mountpoint -q /mnt &&
    die "/mnt is already mounted. Unmount it before starting a destructive installation."

NONINTERACTIVE="${COSTA_INSTALL_NONINTERACTIVE:-0}"

if [[ "${NONINTERACTIVE}" == "1" ]]; then
    DISK="${COSTA_INSTALL_DISK:-}"
    HOSTNAME="${COSTA_INSTALL_HOSTNAME:-archvm}"
    USERNAME="${COSTA_INSTALL_USERNAME:-fcosta}"
    TIMEZONE="${COSTA_INSTALL_TIMEZONE:-Europe/Vienna}"
    KEYBOARD_LAYOUT="${COSTA_INSTALL_KEYBOARD:-pt}"
    CLOCK_FORMAT="${COSTA_INSTALL_CLOCK:-24h}"
    USER_PASS="${COSTA_INSTALL_PASSWORD:-}"
    [[ -n "${DISK}" ]] || die "COSTA_INSTALL_DISK is required in noninteractive mode."
    [[ -n "${USER_PASS}" ]] || die "COSTA_INSTALL_PASSWORD is required in noninteractive mode."
else
    printf '%bAvailable whole disks:%b\n' "${YELLOW}" "${NC}"
    lsblk -d -o NAME,SIZE,TYPE,MODEL,TRAN
    printf '\n'

    read -r -p "Target whole disk (for example /dev/vda or /dev/nvme0n1): " DISK
    read -r -p "Hostname [archvm]: " HOSTNAME
    HOSTNAME="${HOSTNAME:-archvm}"
    read -r -p "Username [fcosta]: " USERNAME
    USERNAME="${USERNAME:-fcosta}"
    read -r -p "Timezone [Europe/Vienna]: " TIMEZONE
    TIMEZONE="${TIMEZONE:-Europe/Vienna}"
    read -r -p "Keyboard layout [pt]: " KEYBOARD_LAYOUT
    KEYBOARD_LAYOUT="${KEYBOARD_LAYOUT:-pt}"
    read -r -p "Clock format (12h/24h) [24h]: " CLOCK_FORMAT
    CLOCK_FORMAT="${CLOCK_FORMAT:-24h}"
fi

[[ "${HOSTNAME}" =~ ^[A-Za-z0-9]([A-Za-z0-9.-]{0,61}[A-Za-z0-9])?$ ]] ||
    die "Hostname contains invalid characters or has an invalid length."
[[ "${USERNAME}" =~ ^[a-z_][a-z0-9_-]{0,31}$ ]] ||
    die "Username must be lowercase and contain only letters, numbers, '_' or '-'."
[[ -e "/usr/share/zoneinfo/${TIMEZONE}" ]] ||
    die "Timezone '${TIMEZONE}' does not exist on the live ISO."
[[ "${KEYBOARD_LAYOUT}" =~ ^[a-zA-Z0-9,_-]+$ ]] ||
    die "Keyboard layout contains unsupported characters."
[[ "${CLOCK_FORMAT}" == "12h" || "${CLOCK_FORMAT}" == "24h" ]] ||
    die "Clock format must be '12h' or '24h'."

if [[ "${NONINTERACTIVE}" != "1" ]]; then
    read -r -s -p "Password for '${USERNAME}': " USER_PASS
    printf '\n'
    [[ -n "${USER_PASS}" ]] || die "The user password cannot be empty."
    read -r -s -p "Confirm password: " USER_PASS_CONFIRM
    printf '\n'
    [[ "${USER_PASS}" == "${USER_PASS_CONFIRM}" ]] || die "Passwords do not match."
    unset USER_PASS_CONFIRM
fi

validate_target_disk

printf '\n%bDANGER: every partition and all data on %s will be erased.%b\n' \
    "${RED}" "${DISK}" "${NC}"
if [[ "${NONINTERACTIVE}" == "1" ]]; then
    [[ "${COSTA_INSTALL_CONFIRM_DISK:-}" == "${DISK}" ]] ||
        die "Set COSTA_INSTALL_CONFIRM_DISK to the full device path to confirm."
else
    read -r -p "Type the full device path '${DISK}' to confirm: " CONFIRM_DISK
    [[ "${CONFIRM_DISK}" == "${DISK}" ]] || die "Confirmation did not match; cancelled."
    unset CONFIRM_DISK
fi

case "${DISK}" in
    *nvme* | *mmcblk*) PART_EFI="${DISK}p1"; PART_ROOT="${DISK}p2" ;;
    *) PART_EFI="${DISK}1"; PART_ROOT="${DISK}2" ;;
esac

log "[1/7] Partitioning ${DISK}..."
sfdisk --wipe always --wipe-partitions always "${DISK}" <<'SFDISK'
label: gpt
size=1G, type=uefi
type=linux
SFDISK
partprobe "${DISK}"
udevadm settle
wait_for_partition "${PART_EFI}"
wait_for_partition "${PART_ROOT}"

log "[2/7] Formatting partitions..."
mkfs.fat -F 32 -n EFI "${PART_EFI}"
mkfs.ext4 -F -L arch-root "${PART_ROOT}"

log "[3/7] Mounting filesystems..."
mount "${PART_ROOT}" /mnt
TARGET_MOUNTED=true
mount --mkdir "${PART_EFI}" /mnt/boot

log "[4/7] Installing Arch, the desktop stack, and AMD graphics support..."
pacstrap -K /mnt \
    base base-devel linux linux-firmware amd-ucode sudo neovim git \
    grub efibootmgr os-prober \
    networkmanager bluez bluez-utils \
    pipewire pipewire-audio pipewire-pulse pipewire-alsa wireplumber \
    pavucontrol playerctl \
    mesa vulkan-radeon mesa-utils vulkan-tools libva-utils \
    hyprland hyprpaper hyprlock hypridle hyprsunset hyprpolkitagent \
    waybar kitty rofi dunst \
    xdg-desktop-portal-hyprland xdg-desktop-portal-gtk \
    xdg-utils xdg-user-dirs \
    sddm nautilus gvfs udisks2 gnome-keyring desktop-file-utils \
    firefox \
    python python-gobject python-cairo gtk4 libadwaita \
    gobject-introspection upower cliphist \
    grim slurp wl-clipboard brightnessctl libnotify \
    pacman-contrib htop lm_sensors \
    ttf-jetbrains-mono-nerd otf-font-awesome papirus-icon-theme \
    qemu-guest-agent spice-vdagent

log "[5/7] Generating /etc/fstab..."
mkdir -p /mnt/etc
genfstab -U /mnt > /mnt/etc/fstab
grep -Eq '[[:space:]]/[[:space:]]' /mnt/etc/fstab ||
    die "Generated fstab does not contain a root filesystem."
grep -Eq '[[:space:]]/boot[[:space:]]' /mnt/etc/fstab ||
    die "Generated fstab does not contain the EFI filesystem."

log "[6/7] Configuring the installed system..."
arch-chroot /mnt /bin/bash -s -- \
    "${HOSTNAME}" "${USERNAME}" "${TIMEZONE}" "${KEYBOARD_LAYOUT}" <<'CHROOT'
set -Eeuo pipefail

readonly INSTALL_HOSTNAME=$1
readonly INSTALL_USERNAME=$2
readonly INSTALL_TIMEZONE=$3
readonly INSTALL_KEYBOARD=$4
readonly USER_HOME="/home/${INSTALL_USERNAME}"

ln -sf "/usr/share/zoneinfo/${INSTALL_TIMEZONE}" /etc/localtime
hwclock --systohc

# Console keymap approximates the Hyprland XKB layout for early boot prompts.
case "${INSTALL_KEYBOARD}" in
    pt*) VCONSOLE_KEYMAP=pt-latin1 ;;
    us*) VCONSOLE_KEYMAP=us ;;
    de*) VCONSOLE_KEYMAP=de-latin1 ;;
    fr*) VCONSOLE_KEYMAP=fr ;;
    es*) VCONSOLE_KEYMAP=es ;;
    *) VCONSOLE_KEYMAP=us ;;
esac
printf 'KEYMAP=%s\n' "${VCONSOLE_KEYMAP}" > /etc/vconsole.conf
sed -i 's/^#en_US.UTF-8 UTF-8/en_US.UTF-8 UTF-8/' /etc/locale.gen
locale-gen
printf 'LANG=en_US.UTF-8\n' > /etc/locale.conf

printf '%s\n' "${INSTALL_HOSTNAME}" > /etc/hostname
cat > /etc/hosts <<HOSTS
127.0.0.1 localhost
::1       localhost
127.0.1.1 ${INSTALL_HOSTNAME}.localdomain ${INSTALL_HOSTNAME}
HOSTS

useradd --create-home --groups wheel --shell /bin/bash "${INSTALL_USERNAME}"
passwd --lock root

printf '%%wheel ALL=(ALL:ALL) ALL\n' > /etc/sudoers.d/10-wheel
chmod 0440 /etc/sudoers.d/10-wheel
visudo --check --file=/etc/sudoers.d/10-wheel

cat > /etc/profile.d/10-user-local-bin.sh <<'PROFILE'
case ":${PATH}:" in
    *":${HOME}/.local/bin:"*) ;;
    *) export PATH="${HOME}/.local/bin:${PATH}" ;;
esac
PROFILE
chmod 0644 /etc/profile.d/10-user-local-bin.sh

grub-install --target=x86_64-efi --efi-directory=/boot --bootloader-id=GRUB
if grep -q '^#GRUB_DISABLE_OS_PROBER=' /etc/default/grub; then
    sed -i 's/^#GRUB_DISABLE_OS_PROBER=.*/GRUB_DISABLE_OS_PROBER=false/' /etc/default/grub
elif ! grep -q '^GRUB_DISABLE_OS_PROBER=' /etc/default/grub; then
    printf '\nGRUB_DISABLE_OS_PROBER=false\n' >> /etc/default/grub
fi
grub-mkconfig -o /boot/grub/grub.cfg

systemctl enable NetworkManager.service
systemctl enable bluetooth.service
systemctl enable sddm.service
CHROOT

printf '%s:%s\n' "${USERNAME}" "${USER_PASS}" | arch-chroot /mnt chpasswd
unset USER_PASS

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
[[ -d "${SCRIPT_DIR}/dotfiles" ]] ||
    die "The repository's dotfiles directory is missing."

USER_HOME="/mnt/home/${USERNAME}"
mkdir -p "${USER_HOME}/.config" "${USER_HOME}/.local/bin" "${USER_HOME}/.local/share"

while IFS= read -r -d '' dotfile_entry; do
    cp -a "${dotfile_entry}" "${USER_HOME}/.config/"
done < <(
    find "${SCRIPT_DIR}/dotfiles" -mindepth 1 -maxdepth 1 \
        ! -name costa-utils ! -name sddm ! -name mimeapps.list -print0
)

cp -a "${SCRIPT_DIR}/dotfiles/costa-utils" \
    "${USER_HOME}/.local/share/costa-utils"
ln -sfn "../share/costa-utils/costa_utils.py" \
    "${USER_HOME}/.local/bin/costa-utils"
mkdir -p "${USER_HOME}/.local/share/applications"
cp -a \
    "${SCRIPT_DIR}/dotfiles/costa-utils/applications/org.fcosta.CostaUtils.desktop" \
    "${USER_HOME}/.local/share/applications/"
mkdir -p "${USER_HOME}/.local/share/icons/hicolor/scalable/apps"
cp -a "${SCRIPT_DIR}/dotfiles/costa-utils/icons/costa_utils.svg" \
    "${USER_HOME}/.local/share/icons/hicolor/scalable/apps/org.fcosta.CostaUtils.svg"
mkdir -p "${USER_HOME}/.config"
cp -a "${SCRIPT_DIR}/dotfiles/mimeapps.list" "${USER_HOME}/.config/mimeapps.list"

# Install the matched SDDM greeter theme and seed it with the default wallpaper.
mkdir -p /mnt/usr/share/sddm/themes
cp -a "${SCRIPT_DIR}/dotfiles/sddm/costa" /mnt/usr/share/sddm/themes/
cp -a "${SCRIPT_DIR}/dotfiles/themes/fcosta/wallpaper.png" \
    /mnt/usr/share/sddm/themes/costa/background.png
mkdir -p /mnt/etc/sddm.conf.d
cp -a "${SCRIPT_DIR}/dotfiles/sddm/costa.conf" /mnt/etc/sddm.conf.d/costa.conf

log "[7/7] Applying ownership, desktop settings, and the default theme..."
arch-chroot /mnt /bin/bash -s -- \
    "${USERNAME}" "${KEYBOARD_LAYOUT}" "${CLOCK_FORMAT}" <<'CHROOT'
set -Eeuo pipefail

readonly INSTALL_USERNAME=$1
readonly INSTALL_KEYBOARD=$2
readonly INSTALL_CLOCK=$3
readonly USER_HOME="/home/${INSTALL_USERNAME}"

find "${USER_HOME}/.config/scripts" -type f -exec chmod 0755 {} +
chmod 0755 "${USER_HOME}/.local/share/costa-utils/costa_utils.py"
chmod 0755 "${USER_HOME}/.local/share/costa-utils/uninstall.sh"
chown -R "${INSTALL_USERNAME}:${INSTALL_USERNAME}" "${USER_HOME}"

runuser -u "${INSTALL_USERNAME}" -- env \
    HOME="${USER_HOME}" \
    XDG_CONFIG_HOME="${USER_HOME}/.config" \
    "${USER_HOME}/.config/scripts/desktop-settings" \
    --keyboard "${INSTALL_KEYBOARD}" \
    --clock "${INSTALL_CLOCK}"
runuser -u "${INSTALL_USERNAME}" -- env \
    HOME="${USER_HOME}" \
    XDG_CONFIG_HOME="${USER_HOME}/.config" \
    "${USER_HOME}/.config/scripts/theme-select" fcosta
runuser -u "${INSTALL_USERNAME}" -- env \
    HOME="${USER_HOME}" \
    XDG_DATA_HOME="${USER_HOME}/.local/share" \
    update-desktop-database "${USER_HOME}/.local/share/applications"
runuser -u "${INSTALL_USERNAME}" -- env \
    HOME="${USER_HOME}" \
    XDG_CONFIG_HOME="${USER_HOME}/.config" \
    xdg-settings set default-web-browser firefox.desktop || true

# theme-select cannot update SDDM as the desktop user; sync colors as root.
if [[ -f "${USER_HOME}/.config/themes/fcosta/colors.css" ]]; then
    accent="$(sed -n 's/^@define-color soft-blue #\([A-Fa-f0-9]\{6\}\).*/\1/p' \
        "${USER_HOME}/.config/themes/fcosta/colors.css" | head -n1)"
    background="$(sed -n 's/^@define-color background #\([A-Fa-f0-9]\{6\}\).*/\1/p' \
        "${USER_HOME}/.config/themes/fcosta/colors.css" | head -n1)"
    foreground="$(sed -n 's/^@define-color foreground #\([A-Fa-f0-9]\{6\}\).*/\1/p' \
        "${USER_HOME}/.config/themes/fcosta/colors.css" | head -n1)"
    foreground_dim="$(sed -n 's/^@define-color foreground-dim #\([A-Fa-f0-9]\{6\}\).*/\1/p' \
        "${USER_HOME}/.config/themes/fcosta/colors.css" | head -n1)"
    cp "${USER_HOME}/.config/hypr/current_wallpaper.png" \
        /usr/share/sddm/themes/costa/background.png
    cat > /usr/share/sddm/themes/costa/theme.conf <<SDDMTHEME
[General]
background=background.png
title=Welcome
accent=#${accent:-719cd6}
backgroundFill=#${background:-192330}
foreground=#${foreground:-cdcecf}
foregroundDim=#${foreground_dim:-738091}
fontFamily=JetBrainsMono Nerd Font
SDDMTHEME
fi
CHROOT

printf '\n%b%s%b\n' "${CYAN}" \
    "========================================================================" "${NC}"
printf '%bInstallation complete. The target filesystems will now be unmounted.%b\n' \
    "${GREEN}" "${NC}"
printf '%bReboot after removing the installation media.%b\n' "${CYAN}" "${NC}"
