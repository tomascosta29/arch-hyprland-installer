return {
	"williamboman/mason.nvim",
	lazy = false,
	cmd = { "Mason", "MasonInstall", "MasonUninstall", "MasonUpdate", "MasonLog" },
	dependencies = {
		"williamboman/mason-lspconfig.nvim",
		"neovim/nvim-lspconfig",
		"hrsh7th/cmp-nvim-lsp",
	},
	config = function()
		require("mason").setup({
			ui = {
				border = "rounded",
				icons = {
					package_installed = "✓",
					package_pending = "➜",
					package_uninstalled = "✗",
				},
			},
		})

		local function root_with_fallback(markers)
			return function(bufnr, on_dir)
				local filename = vim.api.nvim_buf_get_name(bufnr)
				local root = vim.fs.root(filename, markers)
				on_dir(root or vim.fs.dirname(filename))
			end
		end

		local servers = {
			lua_ls = {
				root_dir = root_with_fallback({
					".luarc.json",
					".luarc.jsonc",
					".stylua.toml",
					"stylua.toml",
					".git",
				}),
				settings = {
					Lua = {
						diagnostics = { globals = { "vim" } },
						workspace = {
							library = vim.api.nvim_get_runtime_file("", true),
							checkThirdParty = false,
						},
					},
				},
			},
			pyright = {
				root_dir = root_with_fallback({
					"pyrightconfig.json",
					"pyproject.toml",
					"requirements.txt",
					".git",
				}),
			},
			bashls = {
				root_dir = root_with_fallback({ ".git" }),
			},
			gopls = {
				settings = {
					gopls = {
						analyses = { unusedparams = true },
						staticcheck = true,
						gofumpt = true,
					},
				},
			},
			ts_ls = {},
		}

		require("mason-lspconfig").setup({
			ensure_installed = vim.tbl_keys(servers),
			automatic_enable = false,
		})

		local capabilities = require("cmp_nvim_lsp").default_capabilities()
		for name, config in pairs(servers) do
			config.capabilities = capabilities
			vim.lsp.config(name, config)
			vim.lsp.enable(name)
		end

		-- lazy.nvim may configure this after a command-line buffer's FileType
		-- event, so reconsider loaded buffers once startup has fully settled.
		vim.api.nvim_create_autocmd("User", {
			pattern = "VeryLazy",
			once = true,
			callback = function()
				for _, bufnr in ipairs(vim.api.nvim_list_bufs()) do
					if vim.api.nvim_buf_is_loaded(bufnr) and vim.bo[bufnr].filetype ~= "" then
						vim.api.nvim_exec_autocmds("FileType", {
							buffer = bufnr,
							group = "nvim.lsp.enable",
						})
					end
				end
			end,
		})

		vim.api.nvim_create_autocmd("LspAttach", {
			desc = "LSP buffer keymaps",
			callback = function(args)
				local map = function(lhs, rhs, desc)
					vim.keymap.set("n", lhs, rhs, {
						buffer = args.buf,
						silent = true,
						desc = desc,
					})
				end

				map("gd", vim.lsp.buf.definition, "Go to definition")
				map("gD", vim.lsp.buf.declaration, "Go to declaration")
				map("gi", vim.lsp.buf.implementation, "Go to implementation")
				map("gr", vim.lsp.buf.references, "Go to references")
				map("K", function()
					vim.lsp.buf.hover({ border = "rounded" })
				end, "Hover documentation")
				map("<leader>rn", vim.lsp.buf.rename, "Rename symbol")
				map("<leader>ca", vim.lsp.buf.code_action, "Code action")
				map("<leader>d", vim.diagnostic.open_float, "Line diagnostics")
				map("]d", function()
					vim.diagnostic.jump({ count = 1, float = true })
				end, "Next diagnostic")
				map("[d", function()
					vim.diagnostic.jump({ count = -1, float = true })
				end, "Previous diagnostic")
			end,
		})
	end,
}
