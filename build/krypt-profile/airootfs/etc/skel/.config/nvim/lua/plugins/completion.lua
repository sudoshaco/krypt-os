-- lua/plugins/completion.lua — nvim-cmp + LuaSnip
return {
  -- Snippet-Engine
  {
    "L3MON4D3/LuaSnip",
    version      = "v2.*",
    build        = "make install_jsregexp",
    dependencies = { "rafamadriz/friendly-snippets" },
    config = function()
      local luasnip = require("luasnip")
      require("luasnip.loaders.from_vscode").lazy_load()
      luasnip.config.setup({
        history                  = true,
        updateevents             = "TextChanged,TextChangedI",
        enable_autosnippets      = false,
        delete_check_events      = "TextChanged",
      })
    end,
  },

  -- Auto-Completion
  {
    "hrsh7th/nvim-cmp",
    event        = "InsertEnter",
    dependencies = {
      "hrsh7th/cmp-nvim-lsp",
      "hrsh7th/cmp-buffer",
      "hrsh7th/cmp-path",
      "hrsh7th/cmp-cmdline",
      "saadparwaiz1/cmp_luasnip",
      "L3MON4D3/LuaSnip",
      "onsails/lspkind.nvim",
    },
    config = function()
      local cmp     = require("cmp")
      local luasnip = require("luasnip")
      local lspkind = require("lspkind")

      cmp.setup({
        snippet = {
          expand = function(args)
            luasnip.lsp_expand(args.body)
          end,
        },
        window = {
          completion    = cmp.config.window.bordered(),
          documentation = cmp.config.window.bordered(),
        },
        mapping = cmp.mapping.preset.insert({
          ["<C-b>"]     = cmp.mapping.scroll_docs(-4),
          ["<C-f>"]     = cmp.mapping.scroll_docs(4),
          ["<C-Space>"] = cmp.mapping.complete(),
          ["<C-e>"]     = cmp.mapping.abort(),
          ["<CR>"]      = cmp.mapping.confirm({ select = false }),
          ["<Tab>"] = cmp.mapping(function(fallback)
            if cmp.visible() then
              cmp.select_next_item()
            elseif luasnip.expand_or_locally_jumpable() then
              luasnip.expand_or_jump()
            else
              fallback()
            end
          end, { "i", "s" }),
          ["<S-Tab>"] = cmp.mapping(function(fallback)
            if cmp.visible() then
              cmp.select_prev_item()
            elseif luasnip.locally_jumpable(-1) then
              luasnip.jump(-1)
            else
              fallback()
            end
          end, { "i", "s" }),
        }),
        sources = cmp.config.sources({
          { name = "nvim_lsp", priority = 1000 },
          { name = "luasnip",  priority = 750 },
          { name = "path",     priority = 500 },
        }, {
          { name = "buffer", priority = 250, keyword_length = 3 },
        }),
        formatting = {
          format = lspkind.cmp_format({
            mode        = "symbol_text",
            maxwidth    = 50,
            ellipsis_char = "…",
            symbol_map  = {
              Text        = "󰉿",
              Method      = "󰆧",
              Function    = "󰊕",
              Constructor = "",
              Field       = "󰜢",
              Variable    = "󰀫",
              Class       = "󰠱",
              Interface   = "",
              Module      = "",
              Property    = "󰜢",
              Unit        = "󰑭",
              Value       = "󰎠",
              Enum        = "",
              Keyword     = "󰌋",
              Snippet     = "",
              Color       = "󰏘",
              File        = "󰈙",
              Reference   = "󰈇",
              Folder      = "󰉋",
              EnumMember  = "",
              Constant    = "󰏿",
              Struct      = "󰙅",
              Event       = "",
              Operator    = "󰆕",
              TypeParameter = "",
            },
          }),
        },
        experimental = { ghost_text = { hl_group = "CmpGhostText" } },
        sorting = {
          priority_weight = 2,
          comparators = {
            cmp.config.compare.offset,
            cmp.config.compare.exact,
            cmp.config.compare.score,
            cmp.config.compare.recently_used,
            cmp.config.compare.locality,
            cmp.config.compare.kind,
            cmp.config.compare.sort_text,
            cmp.config.compare.length,
            cmp.config.compare.order,
          },
        },
      })

      -- Cmdline: path completion
      cmp.setup.cmdline({ "/", "?" }, {
        mapping = cmp.mapping.preset.cmdline(),
        sources = { { name = "buffer" } },
      })
      cmp.setup.cmdline(":", {
        mapping = cmp.mapping.preset.cmdline(),
        sources = cmp.config.sources(
          { { name = "path" } },
          { { name = "cmdline" } }
        ),
      })
    end,
  },
}
