-- lua/plugins/telescope.lua — Fuzzy Finder
return {
  {
    "nvim-telescope/telescope.nvim",
    cmd          = "Telescope",
    version      = false,
    dependencies = {
      "nvim-lua/plenary.nvim",
      {
        "nvim-telescope/telescope-fzf-native.nvim",
        build = "make",
        cond  = function() return vim.fn.executable("make") == 1 end,
      },
      "nvim-telescope/telescope-ui-select.nvim",
    },
    opts = function()
      local actions = require("telescope.actions")
      return {
        defaults = {
          prompt_prefix   = "  ",
          selection_caret = " ",
          entry_prefix    = "  ",
          border          = true,
          borderchars     = { "─", "│", "─", "│", "╭", "╮", "╯", "╰" },
          layout_strategy = "horizontal",
          layout_config = {
            horizontal = { prompt_position = "top", preview_width = 0.55, results_width = 0.8 },
            vertical   = { mirror = false },
            width      = 0.87,
            height     = 0.80,
            preview_cutoff = 120,
          },
          sorting_strategy = "ascending",
          winblend         = 0,
          file_ignore_patterns = { "^.git/", "^target/", "__pycache__", "%.pyc", "node_modules" },
          path_display     = { "truncate" },
          mappings = {
            i = {
              ["<C-n>"]    = actions.cycle_history_next,
              ["<C-p>"]    = actions.cycle_history_prev,
              ["<C-j>"]    = actions.move_selection_next,
              ["<C-k>"]    = actions.move_selection_previous,
              ["<C-c>"]    = actions.close,
              ["<Down>"]   = actions.move_selection_next,
              ["<Up>"]     = actions.move_selection_previous,
              ["<CR>"]     = actions.select_default,
              ["<C-x>"]    = actions.select_horizontal,
              ["<C-v>"]    = actions.select_vertical,
              ["<C-t>"]    = actions.select_tab,
              ["<C-u>"]    = actions.preview_scrolling_up,
              ["<C-d>"]    = actions.preview_scrolling_down,
              ["<Tab>"]    = actions.toggle_selection + actions.move_selection_worse,
              ["<S-Tab>"]  = actions.toggle_selection + actions.move_selection_better,
              ["<C-q>"]    = actions.send_to_qflist + actions.open_qflist,
              ["<M-q>"]    = actions.send_selected_to_qflist + actions.open_qflist,
            },
            n = {
              ["<esc>"]  = actions.close,
              ["<CR>"]   = actions.select_default,
              ["<C-x>"]  = actions.select_horizontal,
              ["<C-v>"]  = actions.select_vertical,
              ["<C-t>"]  = actions.select_tab,
              ["<Tab>"]  = actions.toggle_selection + actions.move_selection_worse,
              ["<S-Tab>"]= actions.toggle_selection + actions.move_selection_better,
              ["<C-q>"]  = actions.send_to_qflist + actions.open_qflist,
              ["j"]      = actions.move_selection_next,
              ["k"]      = actions.move_selection_previous,
              ["H"]      = actions.move_to_top,
              ["M"]      = actions.move_to_middle,
              ["L"]      = actions.move_to_bottom,
              ["gg"]     = actions.move_to_top,
              ["G"]      = actions.move_to_bottom,
              ["q"]      = actions.close,
              ["?"]      = actions.which_key,
            },
          },
        },
        extensions = {
          fzf = {
            fuzzy                   = true,
            override_generic_sorter = true,
            override_file_sorter    = true,
            case_mode               = "smart_case",
          },
          ["ui-select"] = {
            require("telescope.themes").get_dropdown(),
          },
        },
      }
    end,
    config = function(_, opts)
      local telescope = require("telescope")
      telescope.setup(opts)
      pcall(telescope.load_extension, "fzf")
      pcall(telescope.load_extension, "ui-select")
    end,
  },
}
