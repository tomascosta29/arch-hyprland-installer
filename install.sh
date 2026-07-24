#!/usr/bin/env bash
# ==============================================================================
# Arch Linux + Hyprland Workstation Automated Installer
# Target Architecture: UEFI / GPT with GRUB Dual-Boot Support
# ==============================================================================

set -euo pipefail

GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${CYAN}"
echo "========================================================================"
echo "         Arch Linux + Hyprland Workstation Installer                    "
echo "========================================================================"
echo -e "${NC}"

# Must be run as root in Arch ISO live environment
if [[ $EUID -ne 0 ]]; then
   echo -e "${RED}Error: This script must be run as root in the Arch Linux Live ISO.${NC}"
   exit 1
fi

# Detect storage disks
echo -e "${YELLOW}Available Disks:${NC}"
lsblk -d -n -o NAME,SIZE,TYPE,MODEL | grep -E 'disk' || true
echo ""

# Interactive Inputs
read -rp "Enter target disk (e.g. /dev/vda, /dev/nvme0n1, /dev/sda): " DISK
read -rp "Enter hostname [default: archvm]: " HOSTNAME
HOSTNAME=${HOSTNAME:-archvm}

read -rp "Enter username [default: fcosta]: " USERNAME
USERNAME=${USERNAME:-fcosta}

read -sp "Enter password for user '${USERNAME}' and root: " USER_PASS
echo ""

if [[ ! -b "${DISK}" ]]; then
    echo -e "${RED}Error: Disk ${DISK} does not exist.${NC}"
    exit 1
fi

echo -e "\n${YELLOW}WARNING: All data on ${DISK} will be erased!${NC}"
read -rp "Are you sure you want to proceed? (y/N): " CONFIRM
if [[ "${CONFIRM}" != "y" && "${CONFIRM}" != "Y" ]]; then
    echo "Installation cancelled."
    exit 0
fi

# Partition Naming (handles /dev/vda1 vs /dev/nvme0n1p1)
if [[ "${DISK}" =~ nvme|mmcblk ]]; then
    PART_EFI="${DISK}p1"
    PART_ROOT="${DISK}p2"
else
    PART_EFI="${DISK}1"
    PART_ROOT="${DISK}2"
fi

echo -e "\n${GREEN}[1/6] Partitioning disk ${DISK}...${NC}"
sfdisk "${DISK}" <<EOF
label: gpt
size=1G, type=uefi
type=linux
EOF

# Ensure kernel registers new partitions
partprobe "${DISK}" 2>/dev/null || udevadm settle

echo -e "\n${GREEN}[2/6] Formatting partitions...${NC}"
mkfs.fat -F32 "${PART_EFI}"
mkfs.ext4 -F "${PART_ROOT}"

echo -e "\n${GREEN}[3/6] Mounting filesystems...${NC}"
mount "${PART_ROOT}" /mnt
mount --mkdir "${PART_EFI}" /mnt/boot

echo -e "\n${GREEN}[4/6] Installing base system & Hyprland packages...${NC}"
pacstrap -K /mnt \
    base linux linux-firmware base-devel neovim git networkmanager \
    qemu-guest-agent spice-vdagent \
    grub efibootmgr os-prober \
    hyprland hyprpaper waybar kitty rofi-wayland dunst polkit-kde-agent \
    xdg-desktop-portal-hyprland xdg-desktop-portal-gtk \
    pipewire pipewire-audio pipewire-pulse pipewire-alsa wireplumber pavucontrol \
    ttf-jetbrains-mono-nerd ttf-font-awesome grim slurp wl-clipboard sddm

echo -e "\n${GREEN}[5/6] Generating fstab...${NC}"
genfstab -U /mnt >> /mnt/fstab

# Copy local dotfiles if available
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ -d "${SCRIPT_DIR}/dotfiles" ]]; then
    echo -e "${GREEN}Copying dotfiles into target root...${NC}"
    mkdir -p "/mnt/home/${USERNAME}/.config"
    cp -r "${SCRIPT_DIR}/dotfiles/"* "/mnt/home/${USERNAME}/.config/"
fi

echo -e "\n${GREEN}[6/6] Configuring installed system...${NC}"
arch-chroot /mnt /bin/bash <<EOF
set -euo pipefail

# Timezone & Hardware Clock
ln -sf /usr/share/zoneinfo/Europe/Lisbon /etc/localtime
hwclock --systohc

# Console Keymap & Locales
echo "KEYMAP=pt-latin1" > /etc/vconsole.conf
sed -i 's/#en_US.UTF-8 UTF-8/en_US.UTF-8 UTF-8/' /etc/locale.gen
locale-gen
echo "LANG=en_US.UTF-8" > /etc/locale.conf

# Hostname
echo "${HOSTNAME}" > /etc/hostname

# Set Root Password
echo "root:${USER_PASS}" | chpasswd

# Create User
useradd -m -G wheel -s /bin/bash "${USERNAME}"
echo "${USERNAME}:${USER_PASS}" | chpasswd

# Sudo Permissions for Wheel Group
echo "%wheel ALL=(ALL:ALL) ALL" > /etc/sudoers.d/10-wheel
chmod 440 /etc/sudoers.d/10-wheel

# Fix Ownership of User Home Directory
chown -R "${USERNAME}:${USERNAME}" "/home/${USERNAME}"

# Apply Default Theme (Catppuccin Macchiato)
su - "${USERNAME}" -c "chmod +x ~/.config/scripts/theme-select && ~/.config/scripts/theme-select catppuccin-macchiato" || true

# Install GRUB EFI & Enable OS-Prober
grub-install --target=x86_64-efi --efi-directory=/boot --bootloader-id=GRUB
echo "GRUB_DISABLE_OS_PROBER=false" >> /etc/default/grub
grub-mkconfig -o /boot/grub/grub.cfg

# Enable Core Systemd Services
systemctl enable NetworkManager
systemctl enable sddm
systemctl enable qemu-guest-agent
systemctl enable spice-vdagentd || true

EOF

echo -e "\n${CYAN}========================================================================${NC}"
echo -e "${GREEN}  SUCCESS! Arch Linux + Hyprland installation complete.${NC}"
echo -e "${CYAN}========================================================================${NC}"
echo -e "You can now exit chroot, unmount (/mnt), and reboot into your system!"
