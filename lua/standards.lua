-- cordanaLLM/praetor Neovim LSP and Tool Configuration
local lspconfig = require("lspconfig")
local configs = require("lspconfig.configs")

if not configs.standards_lsp then
  configs.standards_lsp = {
    default_config = {
      cmd = { "./bin/standards-lsp" },
      filetypes = { "go" },
      root_dir = function(fname)
        return lspconfig.util.root_pattern(".standards.yaml", "meson.build", "go.mod", ".git")(fname)
      end,
      settings = {
        standards = {
          hissEnforcement = true,
          maxLOC = 75,
          maxStatements = 50,
        },
      },
    },
  }
end

lspconfig.standards_lsp.setup({})

-- praetorctl first, the vendored Praetor source tree second, an explicit
-- failure third. Same fallback chain the lefthook governance hooks use, so an
-- editor command and a commit hook cannot disagree about which binary runs.
local function governance_shell(subcommand)
  return "if command -v praetorctl >/dev/null 2>&1; then praetorctl "
    .. subcommand
    .. "; elif [ -d ./cmd/standardsctl ]; then go run ./cmd/standardsctl "
    .. subcommand
    .. "; else echo HISS-16 governance command cannot run because praetorctl is not installed >&2; exit 1; fi"
end

vim.api.nvim_create_user_command("StandardsAudit", function()
  vim.cmd("!" .. governance_shell("audit"))
end, { desc = "Audit repository against declared HISS invariants" })

vim.api.nvim_create_user_command("StandardsCompileContext", function()
  vim.cmd("!" .. governance_shell("compile-context"))
end, { desc = "Compile AGENTS.md cross-agent contexts" })

vim.api.nvim_create_user_command("StandardsVerifyAll", function()
  vim.cmd("!make verify-all")
end, { desc = "Run full standards verification pipeline" })
