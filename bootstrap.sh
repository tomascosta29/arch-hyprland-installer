#!/usr/bin/env bash
# Remote bootstrap installer for Arch Hyprland Workstation.
# Run directly from standard Arch Linux ISO terminal:
#   curl -s https://arch.tomascosta.pt | bash

set -Eeuo pipefail

REPO_OWNER="tomascosta29"
REPO_NAME="arch-hyprland-installer"

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

# Check bootstrap dependencies
command -v curl >/dev/null 2>&1 || die "curl is required."
command -v tar >/dev/null 2>&1 || die "tar is required."
command -v sha256sum >/dev/null 2>&1 || die "sha256sum is required."

# Ask user for flavor choice upfront
printf '%bSelect release flavor to download:%b\n' "${YELLOW}" "${NC}"
printf '  1) Full  - Zsh + Starship + packaged plugins, Neovim with LazyVim (Default)\n'
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
CHECKSUM_URL="${RELEASE_URL}.sha256"

log "Downloading ${FLAVOR} flavor package..."
curl --fail --silent --show-error --location \
    "${RELEASE_URL}" --output "${WORK_DIR}/package.tar.gz" ||
    die "Failed to download the ${FLAVOR} release package."
curl --fail --silent --show-error --location \
    "${CHECKSUM_URL}" --output "${WORK_DIR}/package.tar.gz.sha256" ||
    die "Failed to download the release checksum."

EXPECTED_SHA256="$(tr -d '[:space:]' < "${WORK_DIR}/package.tar.gz.sha256")"
[[ "${EXPECTED_SHA256}" =~ ^[0-9a-fA-F]{64}$ ]] ||
    die "The release checksum is malformed."
printf '%s  %s\n' "${EXPECTED_SHA256}" "${WORK_DIR}/package.tar.gz" |
    sha256sum --check --status ||
    die "Release checksum verification failed."
log "Downloaded and verified the latest GitHub release asset (${FLAVOR})."

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
