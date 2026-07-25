# Migration from Python costa-utils

The Python suite formerly lived at `dotfiles/costa-utils`. Behavioural parity is
now owned by the Rust workspace in this repository under `./costa-utils`.

## Goals

1. Keep the public CLI and D-Bus application id stable.
2. Prefer typed backends in `costa-core`; shell out only where native APIs are not worth it yet.
3. Ship one release binary from `install.sh` / `scripts/deploy-user`.

## Cutover

`scripts/lib/costa-utils.sh` builds `./costa-utils` (override with `COSTA_UTILS_SRC`)
and installs:

- `~/.local/bin/costa-utils`
- desktop entry + icon
- `~/.local/share/costa-utils/icons/`

There is no Python fallback.
