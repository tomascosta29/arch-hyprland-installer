#!/usr/bin/env bash
# Shared helpers for building and installing the Rust costa-utils suite.
# shellcheck shell=bash

costa_utils_source() {
    local root=$1
    if [[ -n "${COSTA_UTILS_SRC:-}" ]]; then
        printf '%s\n' "${COSTA_UTILS_SRC}"
        return 0
    fi
    if [[ -f "${root}/costa-utils/Cargo.toml" ]]; then
        printf '%s\n' "${root}/costa-utils"
        return 0
    fi
    if [[ -f "${root}/../costa-utils/Cargo.toml" ]]; then
        printf '%s\n' "${root}/../costa-utils"
        return 0
    fi
    return 1
}

# Build (if needed) and install the release binary + desktop assets.
# Usage: install_costa_utils <repo-root> <bin-dir> <data-dir> [manifest-file]
install_costa_utils() {
    local repo_root=$1
    local bin_dir=$2
    local data_dir=$3
    local manifest_file=${4:-}
    local src bin desktop icon_src

    if [[ -x "${repo_root}/costa-utils/bin/costa-utils" ]]; then
        src="${repo_root}/costa-utils"
        bin="${src}/bin/costa-utils"
        desktop="${src}/assets/applications/org.fcosta.CostaUtils.desktop"
        icon_src="${src}/assets/icons/costa_utils.svg"
        [[ -f "${desktop}" && -f "${icon_src}" ]] || {
            printf 'Error: missing desktop assets under %s/assets\n' "${src}" >&2
            return 1
        }
    else
        src="$(costa_utils_source "${repo_root}")" || {
            printf 'Error: Rust costa-utils not found. Expected %s/costa-utils\n' \
                "${repo_root}" >&2
            return 1
        }
        bin="${src}/target/release/costa-utils"
        desktop="${src}/assets/applications/org.fcosta.CostaUtils.desktop"
        icon_src="${src}/assets/icons/costa_utils.svg"

        [[ -f "${src}/Cargo.toml" ]] || {
            printf 'Error: missing Cargo.toml in %s\n' "${src}" >&2
            return 1
        }
        [[ -f "${desktop}" && -f "${icon_src}" ]] || {
            printf 'Error: missing desktop assets under %s/assets\n' "${src}" >&2
            return 1
        }

        if [[ ! -x "${bin}" ]]; then
            # shellcheck source=/dev/null
            source "${HOME}/.cargo/env" 2>/dev/null || true
            if ! command -v cargo >/dev/null 2>&1; then
                printf 'Error: cargo is required to build costa-utils\n' >&2
                return 1
            fi
            cargo build --release --manifest-path "${src}/Cargo.toml"
        fi
        [[ -x "${bin}" ]] || {
            printf 'Error: release binary missing at %s\n' "${bin}" >&2
            return 1
        }
    fi

    mkdir -p \
        "${bin_dir}" \
        "${data_dir}/applications" \
        "${data_dir}/icons/hicolor/scalable/apps" \
        "${data_dir}/icons/hicolor/scalable/actions" \
        "${data_dir}/costa-utils/icons"

    install -Dm755 "${bin}" "${bin_dir}/costa-utils"
    install -Dm644 "${desktop}" \
        "${data_dir}/applications/org.fcosta.CostaUtils.desktop"
    install -Dm644 "${icon_src}" \
        "${data_dir}/icons/hicolor/scalable/apps/org.fcosta.CostaUtils.svg"
    if [[ -d "${src}/assets/icons/hicolor" ]]; then
        cp -a "${src}/assets/icons/hicolor/." "${data_dir}/icons/hicolor/"
    fi
    cp -a "${src}/assets/icons/." "${data_dir}/costa-utils/icons/"

    command -v gtk-update-icon-cache >/dev/null 2>&1 &&
        gtk-update-icon-cache -f -t "${data_dir}/icons/hicolor" >/dev/null 2>&1 || true

    if [[ -n "${manifest_file}" ]]; then
        {
            printf 'DATA\tapplications/org.fcosta.CostaUtils.desktop\n'
            printf 'DATA\ticons/hicolor/scalable/apps/org.fcosta.CostaUtils.svg\n'
            printf 'BIN\tcosta-utils\n'
            while IFS= read -r -d '' icon; do
                rel="${icon#"${src}/assets/icons/"}"
                if [[ "${rel}" == hicolor/* ]]; then
                    printf 'DATA\ticons/%s\n' "${rel}"
                else
                    printf 'DATA\tcosta-utils/icons/%s\n' "${rel}"
                fi
            done < <(find "${src}/assets/icons" -type f -print0 | sort -z)
        } >> "${manifest_file}"
    fi
}
