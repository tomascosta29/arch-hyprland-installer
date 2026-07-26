#!/usr/bin/env bash
# Install the Rust costa-utils binary + desktop assets for the current user.

set -Eeuo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="${HOME}/.local/bin"
DATA_DIR="${HOME}/.local/share"
APP_DIR="${DATA_DIR}/costa-utils"
DESKTOP_DIR="${DATA_DIR}/applications"
ICON_DIR="${DATA_DIR}/icons/hicolor/scalable/apps"
ACTION_ICON_DIR="${DATA_DIR}/icons/hicolor/scalable/actions"

source "${HOME}/.cargo/env" 2>/dev/null || true

if ! command -v cargo >/dev/null 2>&1; then
    printf 'cargo not found; install rustup first\n' >&2
    exit 1
fi

# Stop any previous singleton (Python or Rust).
if [[ -x "${BIN_DIR}/costa-utils" ]]; then
    "${BIN_DIR}/costa-utils" --shutdown >/dev/null 2>&1 || true
fi
if command -v pgrep >/dev/null 2>&1; then
    mapfile -t pids < <(pgrep -x costa-utils || true)
    if ((${#pids[@]})); then
        kill "${pids[@]}" 2>/dev/null || true
        sleep 0.2
    fi
fi

cargo build --release --manifest-path "${ROOT}/Cargo.toml"

mkdir -p "${BIN_DIR}" "${APP_DIR}" "${DESKTOP_DIR}" "${ICON_DIR}" "${ACTION_ICON_DIR}"
install -Dm755 "${ROOT}/target/release/costa-utils" "${BIN_DIR}/costa-utils"
install -Dm644 "${ROOT}/assets/applications/org.fcosta.CostaUtils.desktop" \
    "${DESKTOP_DIR}/org.fcosta.CostaUtils.desktop"
install -Dm644 "${ROOT}/assets/icons/costa_utils.svg" \
    "${ICON_DIR}/org.fcosta.CostaUtils.svg"
if [[ -d "${ROOT}/assets/icons/hicolor" ]]; then
    cp -a "${ROOT}/assets/icons/hicolor/." "${DATA_DIR}/icons/hicolor/"
fi
# Keep icons used by blinker UI next to the install tree for reference.
cp -a "${ROOT}/assets/icons/." "${APP_DIR}/icons/" 2>/dev/null || true

command -v update-desktop-database >/dev/null 2>&1 &&
    update-desktop-database "${DESKTOP_DIR}" || true
command -v gtk-update-icon-cache >/dev/null 2>&1 &&
    gtk-update-icon-cache -f -t "${DATA_DIR}/icons/hicolor" >/dev/null 2>&1 || true

printf 'Installed %s\n' "${BIN_DIR}/costa-utils"
"${BIN_DIR}/costa-utils" --help | head -1
