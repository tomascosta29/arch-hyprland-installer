#!/usr/bin/env bash
# Build release packages for GitHub (full and light flavors).

set -Eeuo pipefail

readonly REPOSITORY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly DIST_DIR="${REPOSITORY_ROOT}/dist"

log() {
    printf '\033[0;32m%s\033[0m\n' "$1"
}

# Ensure we are running from the repository root
cd "${REPOSITORY_ROOT}"

# Create dist directory
mkdir -p "${DIST_DIR}"

# Build the costa-utils binary first so we can ship the pre-compiled version
log "Building costa-utils in release mode..."
if ! command -v cargo >/dev/null 2>&1; then
    printf "Error: cargo is required to build release packages.\n" >&2
    exit 1
fi
cargo build --release --manifest-path "${REPOSITORY_ROOT}/costa-utils/Cargo.toml"

# Create a clean temporary directory
TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TEMP_DIR}"' EXIT

# Helper function to build a flavor
build_flavor() {
    local flavor=$1
    local archive_name="arch-hyprland-installer-${flavor}"
    local target_dir="${TEMP_DIR}/${archive_name}"

    log "Packaging ${flavor} flavor..."
    mkdir -p "${target_dir}"

    # Copy files from repository root using git archive to export clean tracked files
    git archive HEAD | tar -x -C "${target_dir}"

    # Create destination for pre-built binary
    mkdir -p "${target_dir}/costa-utils/bin"
    cp -a "${REPOSITORY_ROOT}/costa-utils/target/release/costa-utils" "${target_dir}/costa-utils/bin/costa-utils"

    # Remove Rust source files from the packaged version to prevent shipping source code
    rm -rf "${target_dir}/costa-utils/crates"
    rm -rf "${target_dir}/costa-utils/Cargo.toml"
    rm -rf "${target_dir}/costa-utils/Cargo.lock"

    # Set the default flavor in install.sh
    sed -i "s/^INSTALL_FLAVOR=\"full\"/INSTALL_FLAVOR=\"${flavor}\"/" "${target_dir}/install.sh"

    # If it is the light flavor, remove full-only assets to prevent bloat
    if [[ "${flavor}" == "light" ]]; then
        rm -f "${target_dir}/dotfiles/zshrc"
    fi

    # Create tar.gz archive in the dist directory
    tar -czf "${DIST_DIR}/${archive_name}.tar.gz" -C "${TEMP_DIR}" "${archive_name}"
    log "Created ${DIST_DIR}/${archive_name}.tar.gz"
}

build_flavor "full"
build_flavor "light"

log "Release packaging complete. Files are in ${DIST_DIR}"
