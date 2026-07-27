-- ==============================================================================
-- UI Enhancements: Bufferline, Indentline & Dressing
-- ==============================================================================

return {
	-- Top bufferline
	{
		"akinsho/bufferline.nvim",
		version = "*",
		dependencies = { "nvim-tree/nvim-web-devicons" },
		event = { "BufReadPre", "BufNewFile" },
		config = function()
			require("bufferline").setup({
				options = {
					mode = "buffers",
					diagnostics = "nvim_lsp",
					separator_style = "slant",
					always_show_bufferline = true,
					show_buffer_close_icons = true,
					show_close_icon = false,
				},
			})

			local function delete_buffer()
				local current = vim.api.nvim_get_current_buf()
				local alternate = vim.fn.bufnr("#")

				if vim.bo[current].modified then
					vim.notify("Buffer has unsaved changes", vim.log.levels.WARN)
					return
				end

				if alternate > 0 and vim.api.nvim_buf_is_valid(alternate) then
					vim.api.nvim_set_current_buf(alternate)
				else
					vim.cmd("bnext")
				end

				if vim.api.nvim_buf_is_valid(current) then
					vim.api.nvim_buf_delete(current, {})
				end
			end

			local map = vim.keymap.set
			map("n", "<S-h>", "<CMD>BufferLineCyclePrev<CR>", { desc = "Previous Buffer" })
			map("n", "<S-l>", "<CMD>BufferLineCycleNext<CR>", { desc = "Next Buffer" })
			map("n", "<leader>bd", delete_buffer, { desc = "Delete Buffer" })
		end,
	},

	-- Indent guides
	{
		"lukas-reineke/indent-blankline.nvim",
		main = "ibl",
		event = { "BufReadPre", "BufNewFile" },
		config = function()
			require("ibl").setup({
				indent = { char = "│" },
				scope = { enabled = true },
			})
		end,
	},
}
