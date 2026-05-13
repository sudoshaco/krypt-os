-- lua/config/autocmds.lua — Autocommands

local function augroup(name)
  return vim.api.nvim_create_augroup("krypt_" .. name, { clear = true })
end

-- Cursor-Position beim Öffnen wiederherstellen
vim.api.nvim_create_autocmd("BufReadPost", {
  group = augroup("last_cursor"),
  callback = function()
    local mark = vim.api.nvim_buf_get_mark(0, '"')
    local lcount = vim.api.nvim_buf_line_count(0)
    if mark[1] > 0 and mark[1] <= lcount then
      pcall(vim.api.nvim_win_set_cursor, 0, mark)
    end
  end,
})

-- Trailing Whitespace beim Speichern entfernen
vim.api.nvim_create_autocmd("BufWritePre", {
  group = augroup("trim_whitespace"),
  pattern = { "*.rs", "*.py", "*.lua", "*.sh", "*.toml", "*.json", "*.md" },
  callback = function()
    local cursor = vim.api.nvim_win_get_cursor(0)
    vim.cmd([[silent! %s/\s\+$//e]])
    vim.api.nvim_win_set_cursor(0, cursor)
  end,
})

-- Relativer Zeilennummerierung: absolut im Insert/Command-Mode
vim.api.nvim_create_autocmd({ "InsertEnter" }, {
  group = augroup("relative_number"),
  callback = function() vim.opt.relativenumber = false end,
})
vim.api.nvim_create_autocmd({ "InsertLeave" }, {
  group = augroup("relative_number_restore"),
  callback = function() vim.opt.relativenumber = true end,
})

-- Zeilen-Länge für Rust: 100, Python: 88 (Black-Kompatibel)
vim.api.nvim_create_autocmd("FileType", {
  group   = augroup("colorcolumn"),
  pattern = { "rust" },
  callback = function() vim.opt_local.colorcolumn = "100" end,
})
vim.api.nvim_create_autocmd("FileType", {
  group   = augroup("colorcolumn_python"),
  pattern = { "python" },
  callback = function() vim.opt_local.colorcolumn = "88" end,
})

-- TOML: 4-Leerzeichen-Einrückung
vim.api.nvim_create_autocmd("FileType", {
  group = augroup("toml_indent"),
  pattern = "toml",
  callback = function()
    vim.opt_local.tabstop    = 2
    vim.opt_local.shiftwidth = 2
  end,
})

-- Terminal ohne Zeilennummern
vim.api.nvim_create_autocmd("TermOpen", {
  group = augroup("terminal"),
  callback = function()
    vim.opt_local.number         = false
    vim.opt_local.relativenumber = false
    vim.opt_local.signcolumn     = "no"
    vim.cmd("startinsert")
  end,
})

-- Highlight bei Yank
vim.api.nvim_create_autocmd("TextYankPost", {
  group = augroup("yank_highlight"),
  callback = function()
    vim.highlight.on_yank({ timeout = 200 })
  end,
})

-- Auto-Format on save (LSP) — nur wenn nicht read-only
vim.api.nvim_create_autocmd("BufWritePre", {
  group = augroup("lsp_format"),
  pattern = { "*.rs", "*.py" },
  callback = function()
    if vim.bo.modifiable and not vim.bo.readonly then
      vim.lsp.buf.format({ async = false, timeout_ms = 2000 })
    end
  end,
})
