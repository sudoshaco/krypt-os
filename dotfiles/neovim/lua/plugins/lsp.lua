-- lua/plugins/lsp.lua — LSP + Mason (Rust + Python)
return {
  -- Mason: LSP-Server-Installer
  {
    "williamboman/mason.nvim",
    cmd  = "Mason",
    keys = { { "<leader>cm", "<cmd>Mason<cr>", desc = "Mason" } },
    opts = {
      ui = {
        border = "rounded",
        icons  = { package_installed = "✓", package_pending = "➜", package_uninstalled = "✗" },
      },
    },
  },

  -- mason-lspconfig: Bridge zwischen Mason und nvim-lspconfig
  {
    "williamboman/mason-lspconfig.nvim",
    dependencies = { "williamboman/mason.nvim" },
    opts = {
      ensure_installed = {
        "rust_analyzer",   -- Rust
        "pyright",         -- Python
        "lua_ls",          -- Lua (für die Config selbst)
        "bashls",          -- Bash
        "taplo",           -- TOML
        "jsonls",          -- JSON
      },
      automatic_installation = true,
    },
  },

  -- nvim-lspconfig: LSP-Konfiguration
  {
    "neovim/nvim-lspconfig",
    event        = { "BufReadPre", "BufNewFile" },
    dependencies = {
      "williamboman/mason.nvim",
      "williamboman/mason-lspconfig.nvim",
      "hrsh7th/cmp-nvim-lsp",
    },
    config = function()
      local lspconfig   = require("lspconfig")
      local capabilities = require("cmp_nvim_lsp").default_capabilities()

      -- Diagnostics-Stil
      -- nvim 0.10+: signs als Tabelle direkt in vim.diagnostic.config({signs={text=…}}).
      -- Die alte sign_define-API ist deprecated und löst bei jedem LSP-Open eine
      -- Notice aus.
      vim.diagnostic.config({
        virtual_text = {
          prefix = "●",
          source = "if_many",
        },
        signs = {
          text = {
            [vim.diagnostic.severity.ERROR] = " ",
            [vim.diagnostic.severity.WARN]  = " ",
            [vim.diagnostic.severity.HINT]  = " ",
            [vim.diagnostic.severity.INFO]  = " ",
          },
        },
        underline   = true,
        update_in_insert = false,
        severity_sort    = true,
        float = {
          border  = "rounded",
          source  = "always",
          header  = "",
          prefix  = "",
        },
      })

      -- on_attach: Keybindings nur wenn LSP aktiv
      local on_attach = function(_, bufnr)
        local opts = { buffer = bufnr, silent = true }
        local map  = vim.keymap.set
        map("n", "gD",         vim.lsp.buf.declaration,   vim.tbl_extend("force", opts, { desc = "Deklaration" }))
        map("n", "gd",         vim.lsp.buf.definition,    vim.tbl_extend("force", opts, { desc = "Definition" }))
        map("n", "K",          vim.lsp.buf.hover,         vim.tbl_extend("force", opts, { desc = "Hover" }))
        map("n", "gi",         vim.lsp.buf.implementation, vim.tbl_extend("force", opts, { desc = "Impl." }))
        map("n", "<C-k>",      vim.lsp.buf.signature_help, vim.tbl_extend("force", opts, { desc = "Signature" }))
        map("n", "<leader>ca", vim.lsp.buf.code_action,   vim.tbl_extend("force", opts, { desc = "Code Action" }))
        map("n", "<leader>rn", vim.lsp.buf.rename,        vim.tbl_extend("force", opts, { desc = "Rename" }))
        map("n", "gr",         vim.lsp.buf.references,    vim.tbl_extend("force", opts, { desc = "Refs" }))
      end

      -- ── Rust Analyzer ─────────────────────────────────────────────────────
      lspconfig.rust_analyzer.setup({
        capabilities = capabilities,
        on_attach    = on_attach,
        settings = {
          ["rust-analyzer"] = {
            -- cargo.loadOutDirsFromCheck wurde durch cargo.buildScripts.enable
            -- ersetzt (rust-analyzer 2023+); alte Form gibt eine Notice je File-Open.
            -- checkOnSave als Tabelle ist ebenfalls deprecated → boolean + dedicated
            -- "check"-Sektion mit Clippy-Args.
            cargo     = {
              allFeatures  = true,
              buildScripts = { enable = true },
            },
            checkOnSave = true,
            check       = {
              command   = "clippy",
              extraArgs = { "--", "-D", "warnings" },
            },
            procMacro = { enable = true },
            inlayHints = {
              bindingModeHints    = { enable = true },
              chainingHints       = { enable = true },
              closureReturnTypeHints = { enable = "with_block" },
              lifetimeElisionHints = { enable = "skip_trivial" },
              parameterHints      = { enable = true },
              typeHints           = { enable = true },
            },
          },
        },
      })

      -- ── Pyright ───────────────────────────────────────────────────────────
      lspconfig.pyright.setup({
        capabilities = capabilities,
        on_attach    = on_attach,
        settings = {
          python = {
            analysis = {
              typeCheckingMode     = "strict",
              autoSearchPaths      = true,
              useLibraryCodeForTypes = true,
              diagnosticMode       = "workspace",
            },
          },
        },
      })

      -- ── Lua LS (für Neovim-Config) ────────────────────────────────────────
      lspconfig.lua_ls.setup({
        capabilities = capabilities,
        on_attach    = on_attach,
        settings = {
          Lua = {
            runtime     = { version = "LuaJIT" },
            workspace   = {
              checkThirdParty = false,
              library        = vim.api.nvim_get_runtime_file("", true),
            },
            diagnostics = { globals = { "vim" } },
            format      = { enable = false },
            telemetry   = { enable = false },
          },
        },
      })

      -- ── Weitere Server ────────────────────────────────────────────────────
      for _, server in ipairs({ "bashls", "taplo", "jsonls" }) do
        lspconfig[server].setup({
          capabilities = capabilities,
          on_attach    = on_attach,
        })
      end
    end,
  },
}
