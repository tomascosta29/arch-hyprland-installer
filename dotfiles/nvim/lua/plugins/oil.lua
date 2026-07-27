-- ==============================================================================
-- File Explorer: Oil.nvim (Edit directory as a buffer)
-- ==============================================================================

return {
	"stevearc/oil.nvim",
	cmd = "Oil",
	keys = {
		{ "<leader>e", "<cmd>Oil<CR>", desc = "Open file explorer" },
		{ "-", "<cmd>Oil<CR>", desc = "Open parent directory" },
	},
	dependencies = { "nvim-tree/nvim-web-devicons" },
	config = function()
		require("oil").setup({
			default_file_explorer = true,
			columns = {
				"icon",
				"permissions",
				"size",
				"mtime",
			},
			view_options = {
				show_hidden = true, -- Show hidden files (dotfiles) by default
			},
		})
	end,
}
