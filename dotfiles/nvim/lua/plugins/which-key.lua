-- ==============================================================================
-- Keybinding Helper: Which-Key
-- ==============================================================================

return {
	"folke/which-key.nvim",
	event = "VeryLazy",
	config = function()
		local wk = require("which-key")
		wk.setup({
			preset = "modern",
		})
		wk.add({
			{ "<leader>b", group = "Buffer" },
			{ "<leader>c", group = "Code" },
			{ "<leader>f", group = "Find" },
			{ "<leader>h", group = "Git hunk" },
			{ "<leader>r", group = "Refactor" },
		})
	end,
}
