-- init.lua — Krypt OS Neovim-Konfiguration
-- lazy.nvim Plugin-Manager, Catppuccin Mocha, LSP für Rust + Python
-- Struktur: lua/config/ (Optionen/Keymaps/Autocmds) + lua/plugins/ (Plugin-Specs)

-- lazy.nvim bootstrap
local lazypath = vim.fn.stdpath("data") .. "/lazy/lazy.nvim"
-- vim.loop ist seit Neovim 0.10 deprecated zugunsten von vim.uv;
-- der ältere Name bleibt für 0.9-Kompatibilität als Fallback.
local uv = vim.uv or vim.loop
if not uv.fs_stat(lazypath) then
  vim.fn.system({
    "git", "clone", "--filter=blob:none",
    "https://github.com/folke/lazy.nvim.git",
    "--branch=stable",
    lazypath,
  })
end
vim.opt.rtp:prepend(lazypath)

-- Leader vor lazy.setup() setzen (Plugins dürfen darauf referenzieren)
vim.g.mapleader      = " "
vim.g.maplocalleader = "\\"

require("config.options")
require("config.autocmds")

require("lazy").setup("plugins", {
  defaults   = { lazy = true },
  install    = { colorscheme = { "catppuccin" } },
  checker    = { enabled = true, notify = false },
  change_detection = { notify = false },
  performance = {
    rtp = {
      disabled_plugins = {
        "gzip", "matchit", "matchparen", "netrwPlugin",
        "tarPlugin", "tohtml", "tutor", "zipPlugin",
      },
    },
  },
  ui = {
    border = "rounded",
    icons  = {
      cmd    = "⌘",
      config = "🛠",
      event  = "📅",
      ft     = "📂",
      init   = "⚙",
      keys   = "🗝",
      plugin = "🔌",
      runtime = "💻",
      source = "📄",
      start  = "🚀",
      task   = "📌",
      lazy   = "💤 ",
    },
  },
})

require("config.keymaps")
