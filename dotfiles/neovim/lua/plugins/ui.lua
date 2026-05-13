-- lua/plugins/ui.lua — UI Plugins (neo-tree, lualine, which-key, gitsigns, …)
return {

  -- ── Datei-Explorer ──────────────────────────────────────────────────────────
  {
    "nvim-neo-tree/neo-tree.nvim",
    branch       = "v3.x",
    cmd          = "Neotree",
    dependencies = {
      "nvim-lua/plenary.nvim",
      "nvim-tree/nvim-web-devicons",
      "MunifTanjim/nui.nvim",
    },
    opts = {
      close_if_last_window = true,
      popup_border_style   = "rounded",
      enable_git_status    = true,
      enable_diagnostics   = true,
      default_component_configs = {
        indent = {
          indent_size   = 2,
          padding       = 1,
          with_markers  = true,
          indent_marker = "│",
          last_indent_marker = "└",
        },
        icon = {
          folder_closed = "",
          folder_open   = "",
          folder_empty  = "",
          default       = "󰈙",
        },
        git_status = {
          symbols = {
            added     = "✚", modified = "", deleted = "✖",
            renamed   = "󰁕", untracked = "", ignored  = "",
            unstaged  = "󰄱", staged   = "", conflict = "",
          },
        },
      },
      window = {
        position = "left",
        width    = 35,
        mappings = {
          ["<space>"] = "none",
          ["<cr>"]    = "open",
          ["l"]       = "open",
          ["h"]       = "close_node",
          ["v"]       = "open_vsplit",
          ["s"]       = "open_split",
          ["a"]       = "add",
          ["d"]       = "delete",
          ["r"]       = "rename",
          ["y"]       = "copy_to_clipboard",
          ["x"]       = "cut_to_clipboard",
          ["p"]       = "paste_from_clipboard",
          ["c"]       = "copy",
          ["m"]       = "move",
          ["q"]       = "close_window",
          ["R"]       = "refresh",
          ["?"]       = "show_help",
        },
      },
      filesystem = {
        filtered_items = {
          visible         = false,
          hide_dotfiles   = false,
          hide_gitignored = true,
        },
        follow_current_file   = { enabled = true },
        use_libuv_file_watcher = true,
      },
    },
  },

  -- ── Statusleiste ────────────────────────────────────────────────────────────
  {
    "nvim-lualine/lualine.nvim",
    event        = "VeryLazy",
    dependencies = { "nvim-tree/nvim-web-devicons" },
    opts = {
      options = {
        theme                = "catppuccin",
        globalstatus         = true,
        disabled_filetypes   = { statusline = { "dashboard", "alpha" } },
        component_separators = { left = "", right = "" },
        section_separators   = { left = "", right = "" },
      },
      sections = {
        lualine_a = { { "mode", icon = "" } },
        lualine_b = {
          { "branch",   icon = "" },
          { "diff",     symbols = { added = " ", modified = " ", removed = " " } },
          { "diagnostics",
            symbols  = { error = " ", warn = " ", info = " ", hint = " " },
            colored  = true,
          },
        },
        lualine_c = {
          { "filename", path = 1, symbols = { modified = "  ", readonly = "", unnamed = "" } },
        },
        lualine_x = {
          { "filetype", icon_only = false },
          { "encoding" },
          { "fileformat" },
        },
        lualine_y = { "progress" },
        lualine_z = { { "location", icon = "" } },
      },
    },
  },

  -- ── Keybinding-Hilfe ────────────────────────────────────────────────────────
  {
    "folke/which-key.nvim",
    event = "VeryLazy",
    opts  = {
      plugins = {
        marks     = true,
        registers = true,
        spelling  = { enabled = true, suggestions = 20 },
      },
      win = { border = "rounded" },
      layout = { align = "center" },
      icons = { mappings = true },
    },
    config = function(_, opts)
      local wk = require("which-key")
      wk.setup(opts)
      wk.add({
        { "<leader>f",  group = "Suchen" },
        { "<leader>g",  group = "Git" },
        { "<leader>b",  group = "Buffer" },
        { "<leader>c",  group = "Code" },
        { "<leader>r",  group = "Rename" },
        { "<leader>s",  group = "Swap" },
        { "<leader>q",  group = "Quit" },
      })
    end,
  },

  -- ── Git Signs (Gutter) ──────────────────────────────────────────────────────
  {
    "lewis6991/gitsigns.nvim",
    event = { "BufReadPre", "BufNewFile" },
    opts  = {
      signs = {
        add          = { text = "▎" },
        change       = { text = "▎" },
        delete       = { text = "" },
        topdelete    = { text = "" },
        changedelete = { text = "▎" },
        untracked    = { text = "▎" },
      },
      current_line_blame = false,
      on_attach = function(buf)
        local gs  = package.loaded.gitsigns
        local map = function(mode, l, r, desc)
          vim.keymap.set(mode, l, r, { buffer = buf, desc = desc })
        end
        map("n", "]g", gs.next_hunk,          "Nächster Hunk")
        map("n", "[g", gs.prev_hunk,          "Vorheriger Hunk")
        map("n", "<leader>gp", gs.preview_hunk, "Hunk-Preview")
        map("n", "<leader>gb", function() gs.blame_line({ full = true }) end, "Blame")
        map("n", "<leader>gd", gs.diffthis,   "Diff")
        map("n", "<leader>gr", gs.reset_hunk, "Hunk rücksetzen")
        map("n", "<leader>gR", gs.reset_buffer, "Buffer rücksetzen")
        map("n", "<leader>gs", gs.stage_hunk, "Hunk stagen")
        map("n", "<leader>gS", gs.stage_buffer, "Buffer stagen")
        map("n", "<leader>gu", gs.undo_stage_hunk, "Hunk unstagen")
      end,
    },
  },

  -- ── Indent-Guides ───────────────────────────────────────────────────────────
  {
    "lukas-reineke/indent-blankline.nvim",
    event = { "BufReadPost", "BufNewFile" },
    main  = "ibl",
    opts  = {
      indent = {
        char      = "│",
        tab_char  = "│",
      },
      scope = {
        enabled   = true,
        show_start = true,
        show_end   = false,
        highlight  = { "Function", "Label" },
      },
      exclude = {
        filetypes = {
          "help", "alpha", "dashboard", "neo-tree",
          "Trouble", "trouble", "lazy", "mason",
          "notify", "toggleterm", "lazyterm",
        },
      },
    },
  },

  -- ── Notifications ───────────────────────────────────────────────────────────
  {
    "rcarriga/nvim-notify",
    opts = {
      timeout    = 3000,
      max_height = function() return math.floor(vim.o.lines * 0.75) end,
      max_width  = function() return math.floor(vim.o.columns * 0.75) end,
      on_open    = function(win)
        vim.api.nvim_win_set_config(win, { zindex = 100 })
      end,
      render    = "compact",
      stages    = "fade_in_slide_out",
      top_down  = false,
    },
    init = function()
      vim.notify = require("notify")
    end,
  },

  -- ── Command-Line / Noice ────────────────────────────────────────────────────
  {
    "folke/noice.nvim",
    event        = "VeryLazy",
    dependencies = { "MunifTanjim/nui.nvim", "rcarriga/nvim-notify" },
    opts = {
      lsp = {
        override = {
          ["vim.lsp.util.convert_input_to_markdown_lines"] = true,
          ["vim.lsp.util.stylize_markdown"]                = true,
          ["cmp.entry.get_documentation"]                  = true,
        },
        progress = { enabled = true },
        hover    = { enabled = true },
        signature = { enabled = true },
      },
      routes = {
        { filter = { event = "msg_show", any = { { find = "%d+L, %d+B" }, { find = "; after #%d+" }, { find = "; before #%d+" } } }, view = "mini" },
      },
      presets = {
        bottom_search        = true,
        command_palette      = true,
        long_message_to_split = true,
        inc_rename           = true,
        lsp_doc_border       = true,
      },
    },
  },

  -- ── LazyGit-Integration ─────────────────────────────────────────────────────
  {
    "kdheepak/lazygit.nvim",
    cmd          = "LazyGit",
    dependencies = { "nvim-lua/plenary.nvim" },
    keys         = { { "<leader>gg", "<cmd>LazyGit<cr>", desc = "LazyGit" } },
  },

  -- ── Dashboard ───────────────────────────────────────────────────────────────
  {
    "nvimdev/dashboard-nvim",
    event        = "VimEnter",
    dependencies = { "nvim-tree/nvim-web-devicons" },
    opts = {
      theme = "doom",
      config = {
        header = {
          "",
          "  ██╗  ██╗██████╗ ██╗   ██╗██████╗ ████████╗",
          "  ██║ ██╔╝██╔══██╗╚██╗ ██╔╝██╔══██╗╚══██╔══╝",
          "  █████╔╝ ██████╔╝ ╚████╔╝ ██████╔╝   ██║   ",
          "  ██╔═██╗ ██╔══██╗  ╚██╔╝  ██╔═══╝    ██║   ",
          "  ██║  ██╗██║  ██║   ██║   ██║         ██║   ",
          "  ╚═╝  ╚═╝╚═╝  ╚═╝   ╚═╝   ╚═╝         ╚═╝   ",
          "             OS — Secure by Design",
          "",
        },
        center = {
          { icon = "  ", key = "f", desc = "Dateien suchen",    action = "Telescope find_files" },
          { icon = "  ", key = "r", desc = "Zuletzt geöffnet",  action = "Telescope oldfiles" },
          { icon = "  ", key = "g", desc = "Grep",              action = "Telescope live_grep" },
          { icon = "  ", key = "e", desc = "Explorer",          action = "Neotree toggle" },
          { icon = "  ", key = "q", desc = "Beenden",           action = "qa" },
        },
        footer = function()
          local stats = require("lazy").stats()
          return { "⚡ " .. stats.loaded .. "/" .. stats.count .. " Plugins geladen" }
        end,
      },
    },
  },

  -- ── Devicons ─────────────────────────────────────────────────────────────────
  { "nvim-tree/nvim-web-devicons", lazy = true },

  -- ── Trouble (Diagnostics-Panel) ──────────────────────────────────────────────
  {
    "folke/trouble.nvim",
    cmd          = { "Trouble", "TroubleToggle" },
    dependencies = { "nvim-tree/nvim-web-devicons" },
    keys = {
      { "<leader>xx", "<cmd>Trouble diagnostics toggle<cr>",                        desc = "Diagnostics" },
      { "<leader>xX", "<cmd>Trouble diagnostics toggle filter.buf=0<cr>",           desc = "Buffer Diagnostics" },
      { "<leader>xs", "<cmd>Trouble symbols toggle focus=false<cr>",                desc = "Symbole" },
      { "<leader>xl", "<cmd>Trouble lsp toggle focus=false win.position=right<cr>", desc = "LSP Definitions" },
      { "<leader>xq", "<cmd>Trouble qflist toggle<cr>",                             desc = "Quickfix" },
    },
    opts = { use_diagnostic_signs = true },
  },

  -- ── Todo-Kommentare ──────────────────────────────────────────────────────────
  {
    "folke/todo-comments.nvim",
    event        = { "BufReadPost", "BufNewFile" },
    dependencies = { "nvim-lua/plenary.nvim" },
    keys = {
      { "<leader>xt", "<cmd>Trouble todo toggle<cr>", desc = "Todo (Trouble)" },
      { "]t",  function() require("todo-comments").jump_next() end, desc = "Nächstes Todo" },
      { "[t",  function() require("todo-comments").jump_prev() end, desc = "Vorheriges Todo" },
    },
    opts = { signs = true },
  },
}
