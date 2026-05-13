-- lua/config/keymaps.lua — Globale Keybindings (nach lazy.setup)

local map = vim.keymap.set

-- ── Normal Mode ─────────────────────────────────────────────────────────────
-- Navigation
map("n", "<C-h>", "<C-w>h",        { desc = "Fenster links" })
map("n", "<C-j>", "<C-w>j",        { desc = "Fenster unten" })
map("n", "<C-k>", "<C-w>k",        { desc = "Fenster oben" })
map("n", "<C-l>", "<C-w>l",        { desc = "Fenster rechts" })

-- Resize
map("n", "<C-Up>",    "<cmd>resize +2<cr>",          { desc = "Höhe +" })
map("n", "<C-Down>",  "<cmd>resize -2<cr>",          { desc = "Höhe -" })
map("n", "<C-Left>",  "<cmd>vertical resize -2<cr>", { desc = "Breite -" })
map("n", "<C-Right>", "<cmd>vertical resize +2<cr>", { desc = "Breite +" })

-- Buffer-Navigation
map("n", "<S-h>", "<cmd>bprevious<cr>", { desc = "Vorheriger Buffer" })
map("n", "<S-l>", "<cmd>bnext<cr>",     { desc = "Nächster Buffer" })
map("n", "<leader>bd", "<cmd>bdelete<cr>", { desc = "Buffer schließen" })

-- Datei-Explorer
map("n", "<leader>e",  "<cmd>Neotree toggle<cr>",    { desc = "Explorer" })
map("n", "<leader>fe", "<cmd>Neotree reveal<cr>",    { desc = "Explorer (current)" })

-- Telescope
map("n", "<leader>ff", "<cmd>Telescope find_files<cr>",                { desc = "Dateien suchen" })
map("n", "<leader>fg", "<cmd>Telescope live_grep<cr>",                 { desc = "Grep" })
map("n", "<leader>fb", "<cmd>Telescope buffers<cr>",                   { desc = "Buffer" })
map("n", "<leader>fh", "<cmd>Telescope help_tags<cr>",                 { desc = "Help" })
map("n", "<leader>fr", "<cmd>Telescope oldfiles<cr>",                  { desc = "Zuletzt geöffnet" })
map("n", "<leader>fs", "<cmd>Telescope lsp_document_symbols<cr>",      { desc = "Symbole" })
map("n", "<leader>fw", "<cmd>Telescope lsp_workspace_symbols<cr>",     { desc = "Workspace-Symbole" })
map("n", "<leader>fd", "<cmd>Telescope diagnostics<cr>",               { desc = "Diagnostics" })
map("n", "<leader>gc", "<cmd>Telescope git_commits<cr>",               { desc = "Git-Commits" })
map("n", "<leader>gs", "<cmd>Telescope git_status<cr>",                { desc = "Git-Status" })

-- LSP (ohne Plugin — Fallback für before-lsp-attach)
map("n", "K",          vim.lsp.buf.hover,             { desc = "Hover-Docs" })
map("n", "gd",         vim.lsp.buf.definition,        { desc = "Gehe zu Definition" })
map("n", "gr",         vim.lsp.buf.references,        { desc = "Referenzen" })
map("n", "gi",         vim.lsp.buf.implementation,    { desc = "Implementierung" })
map("n", "<leader>ca", vim.lsp.buf.code_action,       { desc = "Code-Action" })
map("n", "<leader>rn", vim.lsp.buf.rename,            { desc = "Umbenennen" })
map("n", "<leader>D",  vim.lsp.buf.type_definition,   { desc = "Typ-Definition" })
map("n", "[d",         vim.diagnostic.goto_prev,      { desc = "Vorheriger Diagnostic" })
map("n", "]d",         vim.diagnostic.goto_next,      { desc = "Nächster Diagnostic" })
map("n", "<leader>q",  vim.diagnostic.setloclist,     { desc = "Diagnostic-Liste" })

-- Git (Lazygit)
map("n", "<leader>gg", "<cmd>LazyGit<cr>", { desc = "LazyGit" })

-- Suche löschen
map("n", "<Esc>", "<cmd>nohlsearch<cr>", { desc = "Suche löschen" })

-- Speichern
map({ "i", "x", "n", "s" }, "<C-s>", "<cmd>w<cr><esc>", { desc = "Speichern" })

-- Quit
map("n", "<leader>qq", "<cmd>qa<cr>", { desc = "Alle schließen" })

-- ── Visual Mode ─────────────────────────────────────────────────────────────
-- Einrücken ohne Selection zu verlieren
map("v", "<", "<gv", { desc = "Einrücken links" })
map("v", ">", ">gv", { desc = "Einrücken rechts" })

-- Zeilen verschieben
map("v", "J", ":m '>+1<cr>gv=gv", { desc = "Zeile runter" })
map("v", "K", ":m '<-2<cr>gv=gv", { desc = "Zeile rauf" })

-- ── Insert Mode ─────────────────────────────────────────────────────────────
map("i", "jj", "<Esc>", { desc = "Escape" })

-- ── Terminal Mode ────────────────────────────────────────────────────────────
map("t", "<Esc><Esc>", "<C-\\><C-n>", { desc = "Terminal: Normal Mode" })
map("t", "<C-h>", "<cmd>wincmd h<cr>", { desc = "Terminal: Fenster links" })
map("t", "<C-j>", "<cmd>wincmd j<cr>", { desc = "Terminal: Fenster unten" })
map("t", "<C-k>", "<cmd>wincmd k<cr>", { desc = "Terminal: Fenster oben" })
map("t", "<C-l>", "<cmd>wincmd l<cr>", { desc = "Terminal: Fenster rechts" })
