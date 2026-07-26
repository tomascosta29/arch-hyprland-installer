#!/usr/bin/env bash
# Arch Linux + Hyprland workstation installer (UEFI/GPT, destructive).

set -Eeuo pipefail

readonly GREEN='\033[0;32m'
readonly CYAN='\033[0;36m'
readonly YELLOW='\033[1;33m'
readonly RED='\033[0;31m'
readonly NC='\033[0m'
readonly MIN_DISK_BYTES=$((20 * 1024 * 1024 * 1024))
readonly LAZYVIM_STARTER_COMMIT=803bc181d7c0d6d5eeba9274d9be49b287294d99

TARGET_MOUNTED=false
INSTALL_FLAVOR="full"


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
        arch-chroot curl findmnt genfstab lsblk mkfs.ext4 mkfs.fat mount \
        mountpoint pacstrap partprobe readlink sfdisk swapon timedatectl udevadm umount; do
        command -v "${command_name}" >/dev/null 2>&1 ||
            die "Required command '${command_name}' is unavailable. Boot the current Arch ISO."
    done
}

preflight_installation() {
    local current_year ntp_state

    log "Running network, repository, and clock preflight checks..."
    timedatectl set-ntp true >/dev/null 2>&1 ||
        warn "Could not explicitly enable live-environment time synchronization."
    ntp_state="$(timedatectl show --property=NTPSynchronized --value 2>/dev/null || true)"
    current_year="$(date +%Y)"
    [[ "${current_year}" =~ ^[0-9]{4}$ ]] && ((current_year >= 2025)) ||
        die "The live environment clock is invalid. Correct it before installing."
    [[ "${ntp_state}" == true ]] ||
        warn "The clock is plausible but NTP has not reported synchronization yet."
    curl --fail --silent --show-error --location --max-time 15 \
        --output /dev/null \
        https://geo.mirror.pkgbuild.com/core/os/x86_64/core.db ||
        die "Could not reach an Arch package mirror. Fix networking before installation."
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

discover_partition() {
    local disk=$1 expected_number=$2 attempt name part_number

    for attempt in {1..20}; do
        while read -r name part_number; do
            if [[ "${part_number:-}" == "${expected_number}" && -b "${name}" ]]; then
                printf '%s\n' "${name}"
                return 0
            fi
        done < <(lsblk -lnpo NAME,PARTN "${disk}")
        udevadm settle
        sleep 0.25
    done
    die "Could not discover partition ${expected_number} below '${disk}'."
}

verify_partition_layout() {
    local disk=$1 efi_partition=$2 root_partition=$3
    local name part_number part_type partition_count=0
    local efi_guid=c12a7328-f81f-11d2-ba4b-00a0c93ec93b
    local linux_guid=0fc63daf-8483-4772-8e79-3d69d8477de4

    while read -r name part_number part_type; do
        [[ "${part_number:-}" =~ ^[0-9]+$ ]] || continue
        partition_count=$((partition_count + 1))
        case "${part_number}" in
            1)
                [[ "${name}" == "${efi_partition}" &&
                    "${part_type,,}" == "${efi_guid}" ]] ||
                    die "Partition 1 is not the expected EFI System Partition."
                ;;
            2)
                [[ "${name}" == "${root_partition}" &&
                    "${part_type,,}" == "${linux_guid}" ]] ||
                    die "Partition 2 is not the expected Linux root partition."
                ;;
            *) die "Unexpected partition ${part_number} appeared on '${disk}'." ;;
        esac
    done < <(lsblk -lnpo NAME,PARTN,PARTTYPE "${disk}")
    ((partition_count == 2)) ||
        die "Expected exactly two target partitions; found ${partition_count}."
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

printf '%bAvailable whole disks:%b\n' "${YELLOW}" "${NC}"
lsblk -d -o NAME,SIZE,TYPE,MODEL,TRAN
printf '\n'

DEFAULT_INSTALL_PROFILE=bare-metal
if command -v systemd-detect-virt >/dev/null 2>&1 &&
    systemd-detect-virt --vm --quiet; then
    DEFAULT_INSTALL_PROFILE=vm
fi

read -r -p \
    "Installation profile (bare-metal/vm) [${DEFAULT_INSTALL_PROFILE}]: " \
    INSTALL_PROFILE
INSTALL_PROFILE="${INSTALL_PROFILE:-${DEFAULT_INSTALL_PROFILE}}"
case "${INSTALL_PROFILE}" in
    bare-metal) DEFAULT_HOSTNAME=archlinux ;;
    vm) DEFAULT_HOSTNAME=archvm ;;
    *) die "Installation profile must be 'bare-metal' or 'vm'." ;;
esac

read -r -p \
    "Installation flavor (full/light) [${INSTALL_FLAVOR}]: " SELECTED_FLAVOR
INSTALL_FLAVOR="${SELECTED_FLAVOR:-${INSTALL_FLAVOR}}"
if [[ "${INSTALL_FLAVOR}" != "full" && "${INSTALL_FLAVOR}" != "light" ]]; then
    die "Installation flavor must be 'full' or 'light'."
fi

read -r -p "Target whole disk (for example /dev/vda or /dev/nvme1n1): " DISK
read -r -p "Hostname [${DEFAULT_HOSTNAME}]: " HOSTNAME

HOSTNAME="${HOSTNAME:-${DEFAULT_HOSTNAME}}"
read -r -p "Username [fcosta]: " USERNAME
USERNAME="${USERNAME:-fcosta}"
read -r -p "Timezone [Europe/Vienna]: " TIMEZONE
TIMEZONE="${TIMEZONE:-Europe/Vienna}"
read -r -p "Keyboard layout [pt]: " KEYBOARD_LAYOUT
KEYBOARD_LAYOUT="${KEYBOARD_LAYOUT:-pt}"
read -r -p "Clock format (12h/24h) [24h]: " CLOCK_FORMAT
CLOCK_FORMAT="${CLOCK_FORMAT:-24h}"

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

validate_target_disk
preflight_installation

read -r -s -p "Password for '${USERNAME}': " USER_PASS
printf '\n'
[[ -n "${USER_PASS}" ]] || die "The user password cannot be empty."
read -r -s -p "Confirm password: " USER_PASS_CONFIRM
printf '\n'
[[ "${USER_PASS}" == "${USER_PASS_CONFIRM}" ]] || die "Passwords do not match."
unset USER_PASS_CONFIRM

printf '\n%bDANGER: every partition and all data on %s will be erased.%b\n' \
    "${RED}" "${DISK}" "${NC}"
printf '%bOther disks are not repartitioned; verify that this is the Arch target, not the Windows disk.%b\n' \
    "${YELLOW}" "${NC}"
read -r -p "Type the full device path '${DISK}' to confirm: " CONFIRM_DISK
[[ "${CONFIRM_DISK}" == "${DISK}" ]] || die "Confirmation did not match; cancelled."
unset CONFIRM_DISK

log "[1/7] Partitioning ${DISK}..."
sfdisk --wipe always --wipe-partitions always "${DISK}" <<'SFDISK'
label: gpt
size=1G, type=uefi
type=linux
SFDISK
partprobe "${DISK}"
udevadm settle
PART_EFI="$(discover_partition "${DISK}" 1)"
PART_ROOT="$(discover_partition "${DISK}" 2)"
verify_partition_layout "${DISK}" "${PART_EFI}" "${PART_ROOT}"
wait_for_partition "${PART_EFI}"
wait_for_partition "${PART_ROOT}"

log "[2/7] Formatting partitions..."
mkfs.fat -F 32 -n EFI "${PART_EFI}"
mkfs.ext4 -F -L arch-root "${PART_ROOT}"

log "[3/7] Mounting filesystems..."
mount "${PART_ROOT}" /mnt
TARGET_MOUNTED=true
mount --mkdir "${PART_EFI}" /mnt/boot

COMMON_PACKAGES=(
    base linux linux-firmware sudo neovim git openssh
    grub efibootmgr os-prober dosfstools
    networkmanager firewalld
    pipewire pipewire-audio pipewire-pulse pipewire-alsa wireplumber
    pavucontrol playerctl sound-theme-freedesktop
    mesa mesa-utils vulkan-tools libva-utils
    hyprland hyprpaper hyprlock hypridle hyprsunset hyprshutdown hyprpolkitagent
    quickshell kitty rofi dunst
    xdg-desktop-portal-hyprland xdg-desktop-portal-gtk
    xdg-utils xdg-user-dirs
    sddm nautilus gvfs gvfs-mtp udisks2 gnome-keyring
    desktop-file-utils file-roller
    firefox
    gtk4 libadwaita
    upower cliphist
    grim slurp wl-clipboard libnotify
    pacman-contrib htop lm_sensors man-db man-pages
    jq curl libpulse python zram-generator
    ttf-jetbrains-mono-nerd otf-font-awesome
    noto-fonts noto-fonts-emoji noto-fonts-cjk papirus-icon-theme
)

case "${INSTALL_PROFILE}" in
    bare-metal)
        PROFILE_PACKAGES=(
            amd-ucode vulkan-radeon
            fwupd smartmontools nvme-cli ntfs-3g
            bluez bluez-utils brightnessctl
        )
        ;;
    vm)
        PROFILE_PACKAGES=(qemu-guest-agent spice-vdagent)
        ;;
esac

case "${INSTALL_FLAVOR}" in
    full)
        FLAVOR_PACKAGES=(
            zsh starship zoxide fzf
            zsh-autosuggestions zsh-syntax-highlighting eza
        )
        ;;
    light)
        FLAVOR_PACKAGES=()
        ;;
esac

log "[4/7] Installing Arch and the ${INSTALL_PROFILE} workstation package set..."
pacstrap -K /mnt "${COMMON_PACKAGES[@]}" "${PROFILE_PACKAGES[@]}" "${FLAVOR_PACKAGES[@]}"

log "[5/7] Generating /etc/fstab..."
mkdir -p /mnt/etc
genfstab -U /mnt > /mnt/etc/fstab
grep -Eq '[[:space:]]/[[:space:]]' /mnt/etc/fstab ||
    die "Generated fstab does not contain a root filesystem."
grep -Eq '[[:space:]]/boot[[:space:]]' /mnt/etc/fstab ||
    die "Generated fstab does not contain the EFI filesystem."

log "[6/7] Configuring the installed system..."
arch-chroot /mnt /bin/bash -s -- \
    "${HOSTNAME}" "${USERNAME}" "${TIMEZONE}" "${KEYBOARD_LAYOUT}" \
    "${INSTALL_PROFILE}" "${INSTALL_FLAVOR}" <<'CHROOT'
set -Eeuo pipefail

readonly INSTALL_HOSTNAME=$1
readonly INSTALL_USERNAME=$2
readonly INSTALL_TIMEZONE=$3
readonly INSTALL_KEYBOARD=$4
readonly INSTALL_PROFILE=$5
readonly INSTALL_FLAVOR=$6
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

if [[ "${INSTALL_FLAVOR}" == full ]]; then
    USER_SHELL=/usr/bin/zsh
else
    USER_SHELL=/usr/bin/bash
fi
useradd --create-home --groups wheel --shell "${USER_SHELL}" "${INSTALL_USERNAME}"
passwd --lock root

# Compressed in-memory swap improves responsiveness without reserving disk space.
cat > /etc/systemd/zram-generator.conf <<'ZRAM'
[zram0]
ZRAM

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
systemctl enable firewalld.service
systemctl enable sddm.service
if [[ "${INSTALL_PROFILE}" == bare-metal ]]; then
    systemctl enable bluetooth.service
else
    systemctl enable qemu-guest-agent.service
fi

# SDDM provides the login-time PAM hooks which unlock and start GNOME Keyring.
grep -Eq '^-?auth[[:space:]]+optional[[:space:]]+pam_gnome_keyring\.so' \
    /etc/pam.d/sddm
grep -Eq '^-?session[[:space:]]+optional[[:space:]]+pam_gnome_keyring\.so[[:space:]]+auto_start' \
    /etc/pam.d/sddm

# Keep the login keyring password synchronized when the user runs passwd.
if ! grep -Eq '^password[[:space:]]+optional[[:space:]]+pam_gnome_keyring\.so' \
    /etc/pam.d/passwd; then
    printf 'password optional pam_gnome_keyring.so\n' >> /etc/pam.d/passwd
fi
CHROOT

printf '%s:%s\n' "${USERNAME}" "${USER_PASS}" | arch-chroot /mnt chpasswd
unset USER_PASS

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
[[ -d "${SCRIPT_DIR}/dotfiles" ]] ||
    die "The repository's dotfiles directory is missing."

USER_HOME="/mnt/home/${USERNAME}"
mkdir -p \
    "${USER_HOME}/.config" \
    "${USER_HOME}/.local/bin" \
    "${USER_HOME}/.local/share/keyrings"

while IFS= read -r -d '' dotfile_entry; do
    cp -a "${dotfile_entry}" "${USER_HOME}/.config/"
done < <(
    find "${SCRIPT_DIR}/dotfiles" -mindepth 1 -maxdepth 1 \
        ! -name sddm ! -name mimeapps.list ! -name zshrc ! -name starship -print0
)

ln -sfn "${USER_HOME}/.config/quickshell/costa/scripts/qs-activity" \
    "${USER_HOME}/.local/bin/qs-activity"
mkdir -p "${USER_HOME}/.config"
cp -a "${SCRIPT_DIR}/dotfiles/mimeapps.list" "${USER_HOME}/.config/mimeapps.list"

if [[ "${INSTALL_FLAVOR}" == "full" ]]; then
    log "Configuring Zsh, Starship, packaged plugins, and LazyVim..."

    # Deploy custom zshrc if available
    if [[ -f "${SCRIPT_DIR}/dotfiles/zshrc" ]]; then
        cp "${SCRIPT_DIR}/dotfiles/zshrc" "${USER_HOME}/.zshrc"
    fi
    cp "${SCRIPT_DIR}/dotfiles/starship/starship.toml" \
        "${USER_HOME}/.config/starship.toml"

    # Fetch an immutable LazyVim starter revision for reproducible installs.
    if git init --quiet "${USER_HOME}/.config/nvim" &&
        git -C "${USER_HOME}/.config/nvim" remote add origin \
            https://github.com/LazyVim/starter.git &&
        git -C "${USER_HOME}/.config/nvim" fetch --quiet --depth=1 \
            origin "${LAZYVIM_STARTER_COMMIT}" &&
        git -C "${USER_HOME}/.config/nvim" checkout --quiet --detach FETCH_HEAD; then
        :
    else
        warn "Could not install the pinned LazyVim starter."
    fi
    if [[ -d "${USER_HOME}/.config/nvim" ]]; then
        rm -rf "${USER_HOME}/.config/nvim/.git"
    fi
fi

# Seed the same ownership manifest consumed by scripts/deploy-user. A later
# deployment can therefore remove files retired after the initial installation
# without ever treating unrelated user files as managed.
MANIFEST_DIR="${USER_HOME}/.config/costa"
MANIFEST_FILE="${MANIFEST_DIR}/managed-files"
mkdir -p "${MANIFEST_DIR}"
: > "${MANIFEST_FILE}"
for component in dunst hypr kitty quickshell rofi scripts systemd themes; do
    source_root="${SCRIPT_DIR}/dotfiles/${component}"
    while IFS= read -r -d '' source; do
        relative="${source#"${source_root}/"}"
        [[ "${relative}" != */__pycache__/* && "${relative}" != *.pyc ]] || continue
        printf 'CONFIG\t%s/%s\n' "${component}" "${relative}" >> "${MANIFEST_FILE}"
    done < <(find "${source_root}" \( -type f -o -type l \) -print0)
done
printf 'CONFIG\tmimeapps.list\n' >> "${MANIFEST_FILE}"

# Build costa-utils on the live installer (needs cargo + GTK devel headers)
# if a pre-compiled binary is not present, then install the binary and desktop assets.
if [[ ! -x "${SCRIPT_DIR}/costa-utils/bin/costa-utils" ]]; then
    if ! command -v cargo >/dev/null 2>&1; then
        log "Installing rust toolchain on the live installer to build costa-utils..."
        pacman -Syu --noconfirm --needed rust pkgconf gtk4 libadwaita
    fi
fi
# shellcheck source=scripts/lib/costa-utils.sh
source "${SCRIPT_DIR}/scripts/lib/costa-utils.sh"
HOME="${USER_HOME}" install_costa_utils \
    "${SCRIPT_DIR}" \
    "${USER_HOME}/.local/bin" \
    "${USER_HOME}/.local/share" \
    "${MANIFEST_FILE}" ||
    die "Failed to build/install costa-utils."
printf 'BIN\tqs-activity\n' >> "${MANIFEST_FILE}"
chmod 0644 "${MANIFEST_FILE}"

# Install the matched SDDM greeter theme and seed it with the default wallpaper.
mkdir -p /mnt/usr/share/sddm/themes
cp -a "${SCRIPT_DIR}/dotfiles/sddm/costa" /mnt/usr/share/sddm/themes/
cp -a "${SCRIPT_DIR}/dotfiles/themes/fcosta/wallpaper.png" \
    /mnt/usr/share/sddm/themes/costa/background.png
mkdir -p /mnt/etc/sddm.conf.d
cp -a "${SCRIPT_DIR}/dotfiles/sddm/costa.conf" /mnt/etc/sddm.conf.d/costa.conf

log "[7/7] Applying ownership, desktop settings, and the default theme..."
arch-chroot /mnt /bin/bash -s -- \
    "${USERNAME}" "${KEYBOARD_LAYOUT}" "${CLOCK_FORMAT}" "${INSTALL_PROFILE}" <<'CHROOT'
set -Eeuo pipefail

readonly INSTALL_USERNAME=$1
readonly INSTALL_KEYBOARD=$2
readonly INSTALL_CLOCK=$3
readonly INSTALL_PROFILE=$4
readonly USER_HOME="/home/${INSTALL_USERNAME}"

find "${USER_HOME}/.config/scripts" -type f -exec chmod 0755 {} +
find "${USER_HOME}/.config/quickshell/costa/scripts" -type f -exec chmod 0755 {} +
chmod 0755 "${USER_HOME}/.local/bin/costa-utils" "${USER_HOME}/.local/bin/qs-activity"
chown -R "${INSTALL_USERNAME}:${INSTALL_USERNAME}" "${USER_HOME}"

runuser -u "${INSTALL_USERNAME}" -- env \
    HOME="${USER_HOME}" \
    XDG_CONFIG_HOME="${USER_HOME}/.config" \
    xdg-user-dirs-update
runuser -u "${INSTALL_USERNAME}" -- env \
    HOME="${USER_HOME}" \
    XDG_CONFIG_HOME="${USER_HOME}/.config" \
    "${USER_HOME}/.config/scripts/desktop-settings" \
    --keyboard "${INSTALL_KEYBOARD}" \
    --clock "${INSTALL_CLOCK}"
runuser -u "${INSTALL_USERNAME}" -- env \
    HOME="${USER_HOME}" \
    XDG_CONFIG_HOME="${USER_HOME}/.config" \
    COSTA_QUICKSHELL_RELOAD=0 \
    "${USER_HOME}/.config/scripts/quickshell-profile" "${INSTALL_PROFILE}"
runuser -u "${INSTALL_USERNAME}" -- env \
    HOME="${USER_HOME}" \
    XDG_CONFIG_HOME="${USER_HOME}/.config" \
    COSTA_THEME_RELOAD=0 \
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
