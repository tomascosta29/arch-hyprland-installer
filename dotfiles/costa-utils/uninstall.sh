#!/usr/bin/env bash
# Remove the per-user Costa Utils installation. User data is kept by default.

set -Eeuo pipefail

readonly INSTALL_DIR="${HOME}/.local/share/costa-utils"
readonly BIN_PATH="${HOME}/.local/bin/costa-utils"
readonly DESKTOP_PATH="${HOME}/.local/share/applications/org.fcosta.CostaUtils.desktop"
readonly ICON_PATH="${HOME}/.local/share/icons/hicolor/scalable/apps/org.fcosta.CostaUtils.svg"
readonly STATE_DIR="${XDG_STATE_HOME:-${HOME}/.local/state}/costa-utils"
readonly CONFIG_DIR="${XDG_CONFIG_HOME:-${HOME}/.config}/costa-utils"
readonly CACHE_DIR="${XDG_CACHE_HOME:-${HOME}/.cache}/costa-utils"

purge=false
if (($# > 1)); then
    printf 'Usage: %s [--purge]\n' "$0" >&2
    exit 2
elif (($# == 1)); then
    [[ "$1" == "--purge" ]] || {
        printf "Unknown option '%s'. Use --purge to remove user data.\n" "$1" >&2
        exit 2
    }
    purge=true
fi

printf 'Uninstalling Costa Utils...\n'
rm -f -- "${BIN_PATH}" "${DESKTOP_PATH}" "${ICON_PATH}"
if [[ -d "${INSTALL_DIR}" ]]; then
    rm -r -- "${INSTALL_DIR}"
fi

if [[ "${purge}" == true ]]; then
    for data_dir in "${STATE_DIR}" "${CONFIG_DIR}" "${CACHE_DIR}"; do
        [[ -d "${data_dir}" ]] && rm -r -- "${data_dir}"
    done
    printf 'Application and user data removed.\n'
else
    printf 'Application removed; user data was preserved. Use --purge to remove it.\n'
fi

command -v update-desktop-database >/dev/null 2>&1 &&
    update-desktop-database "${HOME}/.local/share/applications" || true
