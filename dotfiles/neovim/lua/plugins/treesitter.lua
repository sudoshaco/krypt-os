-- lua/plugins/treesitter.lua — Syntax-Highlighting + Textobjects
return {
  {
    "nvim-treesitter/nvim-treesitter",
    build   = ":TSUpdate",
    event   = { "BufReadPost", "BufNewFile" },
    dependencies = {
      "nvim-treesitter/nvim-treesitter-textobjects",
    },
    opts = {
      ensure_installed = {
        "rust", "python", "lua", "bash",
        "toml", "json", "yaml", "markdown", "markdown_inline",
        "vim", "vimdoc", "c", "regex",
      },
      highlight = {
        enable  = true,
        disable = function(_, buf)
          local max_filesize = 100 * 1024  -- 100 KB
          -- vim.loop seit nvim 0.10 deprecated → vim.uv mit Fallback (siehe init.lua)
          local uv = vim.uv or vim.loop
          local ok, stats = pcall(uv.fs_stat, vim.api.nvim_buf_get_name(buf))
          if ok and stats and stats.size > max_filesize then return true end
        end,
      },
      indent          = { enable = true },
      auto_install    = true,
      textobjects = {
        select = {
          enable    = true,
          lookahead = true,
          keymaps = {
            ["af"] = "@function.outer",
            ["if"] = "@function.inner",
            ["ac"] = "@class.outer",
            ["ic"] = "@class.inner",
            ["aa"] = "@parameter.outer",
            ["ia"] = "@parameter.inner",
            ["ab"] = "@block.outer",
            ["ib"] = "@block.inner",
          },
        },
        move = {
          enable              = true,
          set_jumps           = true,
          goto_next_start     = { ["]f"] = "@function.outer", ["]c"] = "@class.outer" },
          goto_next_end       = { ["]F"] = "@function.outer", ["]C"] = "@class.outer" },
          goto_previous_start = { ["[f"] = "@function.outer", ["[c"] = "@class.outer" },
          goto_previous_end   = { ["[F"] = "@function.outer", ["[C"] = "@class.outer" },
        },
        swap = {
          enable = true,
          swap_next     = { ["<leader>sp"] = "@parameter.inner" },
          swap_previous = { ["<leader>sP"] = "@parameter.inner" },
        },
      },
    },
    config = function(_, opts)
      require("nvim-treesitter.configs").setup(opts)
    end,
  },
}
