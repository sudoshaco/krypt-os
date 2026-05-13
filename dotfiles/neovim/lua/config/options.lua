-- lua/config/options.lua — Vim-Optionen

local opt = vim.opt

-- Darstellung
opt.number         = true
opt.relativenumber = true
opt.cursorline     = true
opt.signcolumn     = "yes"
opt.termguicolors  = true
opt.showmode       = false
opt.laststatus     = 3        -- globale Statusleiste
opt.cmdheight      = 1
opt.pumheight      = 10       -- max Completion-Menü-Zeilen
opt.scrolloff      = 8
opt.sidescrolloff  = 8
opt.wrap           = false

-- Einrückung
opt.tabstop        = 4
opt.shiftwidth     = 4
opt.expandtab      = true
opt.smartindent    = true
opt.shiftround     = true

-- Suche
opt.ignorecase     = true
opt.smartcase      = true
opt.hlsearch       = false
opt.incsearch      = true

-- Dateien
opt.swapfile       = false
opt.backup         = false
opt.undofile       = true
opt.undodir        = vim.fn.stdpath("data") .. "/undo"
opt.autowrite      = true

-- Splits
opt.splitbelow     = true
opt.splitright     = true

-- Performance
opt.updatetime     = 200
opt.timeoutlen     = 300

-- Clipboard
opt.clipboard      = "unnamedplus"

-- Whitespace visualisieren
opt.list           = true
opt.listchars      = { tab = "→ ", trail = "·", nbsp = "·" }

-- Fold (via Treesitter)
opt.foldmethod     = "expr"
opt.foldexpr       = "nvim_treesitter#foldexpr()"
opt.foldlevel      = 99   -- beim Öffnen alle Folds aufgeklappt

-- Autocompletion
opt.completeopt    = { "menu", "menuone", "noselect" }

-- Grep-Programm
if vim.fn.executable("rg") == 1 then
  opt.grepprg    = "rg --vimgrep --smart-case"
  opt.grepformat = "%f:%l:%c:%m"
end

-- Netrw ausblenden (neo-tree stattdessen)
vim.g.loaded_netrw       = 1
vim.g.loaded_netrwPlugin = 1
