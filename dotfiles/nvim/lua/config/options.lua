-- ==============================================================================
-- Neovim General Options
-- ==============================================================================

local opt = vim.opt

-- Line Numbers
opt.number = true -- Show line numbers
opt.relativenumber = true -- Show relative line numbers for fast jumping

-- Tabs & Indentation
opt.tabstop = 4 -- 4 spaces per tab
opt.shiftwidth = 4 -- 4 spaces per indent level
opt.expandtab = true -- Convert tabs to spaces
opt.smartindent = true -- Auto-indent new lines intelligently

-- Search Settings
opt.ignorecase = true -- Case-insensitive searching
opt.smartcase = true -- Unless query contains capital letters
opt.hlsearch = true -- Highlight search matches
opt.incsearch = true -- Show search matches while typing
opt.inccommand = "split" -- Preview substitutions as they are typed

-- Appearance & UI
opt.termguicolors = true -- True color support in terminal
opt.signcolumn = "yes" -- Always show signcolumn (avoids text shifting for LSP/git)
opt.cursorline = true -- Highlight the current line
opt.showmode = false -- Hide default -- INSERT -- text since lualine already shows it!
opt.cmdheight = 0 -- Show the command line only when it is being used
opt.shortmess:append("I") -- Hide default Vim intro splash screen text on startup
opt.wrap = false -- Disable line wrapping by default
opt.scrolloff = 8 -- Keep 8 lines visible above/below cursor when scrolling
opt.sidescrolloff = 8

-- Clipboard & System Integration
opt.clipboard = "unnamedplus" -- Share system clipboard (Wayland/wl-clipboard compatible)
opt.mouse = "a" -- Enable mouse support
opt.confirm = true -- Ask before abandoning an unsaved buffer

-- Splits
opt.splitright = true -- New vertical splits open to the right
opt.splitbelow = true -- New horizontal splits open below

-- Undofile & Backup
opt.undofile = true -- Save undo history to file (persistent undo across reboots!)
opt.updatetime = 250 -- Faster completion & diagnostic popup refresh (default 4000ms)
opt.timeoutlen = 400 -- Responsive mapped-key and which-key menus

vim.diagnostic.config({
	severity_sort = true,
	signs = true,
	underline = true,
	update_in_insert = false,
	virtual_text = {
		spacing = 2,
		source = "if_many",
	},
	float = {
		border = "rounded",
		source = true,
	},
})
