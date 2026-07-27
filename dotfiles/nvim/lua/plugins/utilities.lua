-- ==============================================================================
-- Coding Utilities: Autopairs & Comment
-- ==============================================================================

return {
	-- Auto-close brackets and quotes
	{
		"windwp/nvim-autopairs",
		event = "InsertEnter",
		config = function()
			local autopairs = require("nvim-autopairs")
			autopairs.setup({
				check_ts = true, -- Enable Treesitter integration
			})

			-- Integrate with nvim-cmp autocompletion
			local cmp_autopairs = require("nvim-autopairs.completion.cmp")
			local cmp = require("cmp")
			cmp.event:on("confirm_done", cmp_autopairs.on_confirm_done())
		end,
	},
}
