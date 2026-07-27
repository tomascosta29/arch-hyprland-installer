-- ==============================================================================
-- Colorscheme: Nightfox (matches the active Costa terminal palette)
-- ==============================================================================

return {
	"EdenEast/nightfox.nvim",
	priority = 1000, -- Make sure theme loads first before other plugins
	config = function()
		require("nightfox").setup({
			options = {
				style = "nightfox",
				transparent = false,
				terminal_colors = true,
				styles = {
					comments = "italic",
					keywords = "bold",
					types = "italic,bold",
				},
			},
		})

		-- Load colorscheme
		vim.cmd("colorscheme nightfox")
	end,
}
