-- ==============================================================================
-- Syntax Highlighting: Treesitter
-- ==============================================================================

return {
	"nvim-treesitter/nvim-treesitter",
	event = { "BufReadPost", "BufNewFile" },
	branch = "master",
	build = ":TSUpdate",
	config = function()
		-- Use git clone instead of tarball downloads to avoid extraction errors
		require("nvim-treesitter.install").prefer_git = true

		local treesitter = require("nvim-treesitter.configs")
		treesitter.setup({
			ensure_installed = {
				"go",
				"gomod",
				"gosum",
				"python",
				"bash",
				"lua",
				"vim",
				"vimdoc",
				"json",
				"yaml",
				"markdown",
			},
			auto_install = true,
			highlight = {
				enable = true,
			},
			indent = {
				enable = true,
			},
		})
	end,
}
