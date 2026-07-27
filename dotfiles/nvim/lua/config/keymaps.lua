-- ==============================================================================
-- Neovim Keymaps Configuration
-- ==============================================================================

-- Set Leader key to Space
vim.g.mapleader = " "
vim.g.maplocalleader = " "

local map = vim.keymap.set

-- Clear search highlights with <leader>nh (no highlight)
map("n", "<Esc>", "<cmd>nohlsearch<CR>", { desc = "Clear search highlights", silent = true })

-- Save & Quit shortcuts
map("n", "<leader>w", "<cmd>write<CR>", { desc = "Save file", silent = true })
map("n", "<leader>q", "<cmd>quit<CR>", { desc = "Quit window", silent = true })

-- Easy Split Window Navigation (Ctrl + h/j/k/l)
map("n", "<C-h>", "<C-w>h", { desc = "Move to left split" })
map("n", "<C-j>", "<C-w>j", { desc = "Move to lower split" })
map("n", "<C-k>", "<C-w>k", { desc = "Move to upper split" })
map("n", "<C-l>", "<C-w>l", { desc = "Move to right split" })

-- Resize splits with Arrow Keys
map("n", "<C-Up>", ":resize +2<CR>", { desc = "Increase window height", silent = true })
map("n", "<C-Down>", ":resize -2<CR>", { desc = "Decrease window height", silent = true })
map("n", "<C-Left>", ":vertical resize -2<CR>", { desc = "Decrease window width", silent = true })
map("n", "<C-Right>", ":vertical resize +2<CR>", { desc = "Increase window width", silent = true })

-- Move selected lines up/down in Visual mode (Alt + j/k)
map("v", "J", ":m '>+1<CR>gv=gv", { desc = "Move line down" })
map("v", "K", ":m '<-2<CR>gv=gv", { desc = "Move line up" })

-- Better paste: replace selection without overwriting clipboard register
map("x", "<leader>p", [["_dP]], { desc = "Paste without overwriting clipboard" })

-- Keep the selection while indenting.
map("v", "<", "<gv", { desc = "Indent left" })
map("v", ">", ">gv", { desc = "Indent right" })
