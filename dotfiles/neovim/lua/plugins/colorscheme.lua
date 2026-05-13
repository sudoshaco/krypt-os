-- lua/plugins/colorscheme.lua — Catppuccin Mocha
return {
  {
    "catppuccin/nvim",
    name     = "catppuccin",
    priority = 1000,
    lazy     = false,
    opts = {
      flavour          = "mocha",
      background       = { light = "latte", dark = "mocha" },
      transparent_background = false,
      show_end_of_buffer     = false,
      term_colors            = true,
      dim_inactive = {
        enabled    = true,
        shade      = "dark",
        percentage = 0.10,
      },
      styles = {
        comments    = { "italic" },
        conditionals = { "italic" },
        keywords    = { "bold" },
        functions   = {},
        strings     = {},
        variables   = {},
      },
      integrations = {
        cmp           = true,
        gitsigns      = true,
        neotree       = true,
        telescope     = { enabled = true },
        treesitter    = true,
        which_key     = true,
        lsp_trouble   = true,
        mason         = true,
        noice         = true,
        notify        = true,
        indent_blankline = { enabled = true, scope_color = "lavender" },
        native_lsp = {
          enabled          = true,
          virtual_text = {
            errors      = { "italic" },
            hints       = { "italic" },
            warnings    = { "italic" },
            information = { "italic" },
          },
          underlines = {
            errors      = { "undercurl" },
            hints       = { "underdashed" },
            warnings    = { "undercurl" },
            information = { "underline" },
          },
        },
      },
      -- Krypt-Violet als primäre Akzentfarbe
      custom_highlights = function(colors)
        return {
          -- Krypt-Violet (#9d4edd) statt Catppuccin Mauve (#cba6f7)
          ["@type"]         = { fg = "#9d4edd" },
          ["@function"]     = { fg = "#89b4fa", style = {} },
          CursorLine        = { bg = colors.surface0 },
          LineNr            = { fg = colors.overlay0 },
          CursorLineNr      = { fg = "#9d4edd", style = { "bold" } },
          -- Telescope
          TelescopeSelection = { bg = colors.surface1 },
          TelescopeMatching  = { fg = "#9d4edd", style = { "bold" } },
          -- Which-key
          WhichKeyBorder    = { fg = "#9d4edd" },
        }
      end,
    },
    config = function(_, opts)
      require("catppuccin").setup(opts)
      vim.cmd.colorscheme("catppuccin")
    end,
  },
}
