-- ==============================================================================
-- Neovim Configuration Entrypoint
-- ==============================================================================

-- Load core settings and keymaps
require("config.options")
require("config.keymaps")

-- Load plugin manager & plugins
require("config.lazy")
