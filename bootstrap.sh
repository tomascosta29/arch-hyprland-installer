#!/usr/bin/env bash
# Remote bootstrap installer for Arch Hyprland Workstation.
# Run directly from standard Arch Linux ISO terminal:
#   curl -s https://arch.tomascosta.pt | bash

set -Eeuo pipefail

REPO_OWNER="tomascosta29"
REPO_NAME="arch-hyprland-installer"
BRANCH="quickshell"

# Colors
readonly GREEN='\033[0;32m'
readonly CYAN='\033[0;36m'
readonly YELLOW='\033[1;33m'
readonly RED='\033[0;31m'
readonly NC='\033[0m'

log() { printf '%b%s%b\n' "${GREEN}" "$1" "${NC}"; }
warn() { printf '%bWarning: %s%b\n' "${YELLOW}" "$1" "${NC}" >&2; }
die() { printf '%bError: %s%b\n' "${RED}" "$1" "${NC}" >&2; exit 1; }

(( EUID == 0 )) || die "Run this installer as root from the Arch Linux live ISO."

printf '%b' "${CYAN}"
printf '%s\n' \
    "========================================================================" \
    "         Arch Linux + Hyprland Installer Bootstrap" \
    "========================================================================"
printf '%b\n' "${NC}"

# Check for curl and tar
command -v curl >/dev/null 2>&1 || die "curl is required."
command -v tar >/dev/null 2>&1 || die "tar is required."

# Ask user for flavor choice upfront
printf '%bSelect release flavor to download:%b\n' "${YELLOW}" "${NC}"
printf '  1) Full  - Zsh + Oh My Zsh + Plugins, Neovim with LazyVim starter (Default)\n'
printf '  2) Light - Bash shell, stock Neovim, minimal footprint\n'
read -r -p "Selection [1]: " FLAVOR_CHOICE < /dev/tty || FLAVOR_CHOICE="1"
FLAVOR_CHOICE="${FLAVOR_CHOICE:-1}"

case "${FLAVOR_CHOICE}" in
    1|full|Full) FLAVOR="full" ;;
    2|light|Light) FLAVOR="light" ;;
    *) warn "Invalid selection; defaulting to full."; FLAVOR="full" ;;
esac

# Create temporary workspace
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "${WORK_DIR}"' EXIT

RELEASE_URL="https://github.com/${REPO_OWNER}/${REPO_NAME}/releases/latest/download/arch-hyprland-installer-${FLAVOR}.tar.gz"
FALLBACK_TARBALL_URL="https://github.com/${REPO_OWNER}/${REPO_NAME}/archive/refs/heads/${BRANCH}.tar.gz"

log "Downloading ${FLAVOR} flavor package..."

if curl --fail --silent --show-error --location "${RELEASE_URL}" --output "${WORK_DIR}/package.tar.gz" 2>/dev/null; then
    log "Downloaded latest GitHub release asset (${FLAVOR})."
else
    warn "Latest release asset not found. Downloading repository archive fallback..."
    curl --fail --silent --show-error --location "${FALLBACK_TARBALL_URL}" --output "${WORK_DIR}/package.tar.gz" ||
        die "Failed to download installer package."
fi

log "Extracting package..."
tar -xzf "${WORK_DIR}/package.tar.gz" -C "${WORK_DIR}"

# Find extracted directory containing install.sh
INSTALL_DIR="$(find "${WORK_DIR}" -mindepth 1 -maxdepth 2 -name "install.sh" -exec dirname {} \; | head -n 1)"

[[ -n "${INSTALL_DIR}" && -f "${INSTALL_DIR}/install.sh" ]] ||
    die "Could not find install.sh in downloaded package."

cd "${INSTALL_DIR}"
chmod +x install.sh

# Reconnect stdin to /dev/tty for interactive installer prompts if piped
if [[ ! -t 0 ]]; then
    exec ./install.sh < /dev/tty
else
    exec ./install.sh
fi
