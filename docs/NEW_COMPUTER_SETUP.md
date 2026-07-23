# New Computer Setup

This guide moves the development project, not every piece of local application
state. GitHub is the source of truth for code and durable context.

## 1. Install prerequisites

Install:

- Git
- Node.js with npm
- Rust through rustup
- Visual Studio Build Tools with Desktop development with C++
- Microsoft Edge WebView2 Runtime
- Codex Desktop or a compatible Codex CLI

On Windows, confirm:

```powershell
git --version
node --version
npm --version
rustc --version
cargo --version
codex --version
```

## 2. Choose a recovery path

The simplest option is to open Codex on the new computer and paste the entire
prompt from `docs/NEW_COMPUTER_CODEX_PROMPT.md`. That prompt checks the machine,
clones the repository, installs project dependencies, restores context, and
runs the validation baseline.

To clone manually instead, continue with the steps below.

## 3. Clone the repository

```powershell
git clone git@github.com:liweichao-coder/agent-bar.git
cd agent-bar
npm install
```

If SSH is not configured on the new computer, use the repository HTTPS clone
address or configure a new SSH key first.

## 4. Verify the project

```powershell
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run tauri build -- --debug --no-bundle
```

For interactive development:

```powershell
npm run dev
npm run tauri dev
```

The web development server uses `http://127.0.0.1:1420/`.

## 5. Restore Codex context

Open the cloned repository as a Codex project. Start a new task and paste the
contents of `docs/NEW_COMPUTER_CODEX_PROMPT.md`.

Codex should read, in order:

1. `AGENTS.md`
2. `docs/CURRENT_STATE.md`
3. `README.md`
4. `docs/product-requirements.md`
5. `docs/architecture-sketch.md`
6. the relevant recent files in `docs/iterations/`

Checked-in context is more reliable than depending on an old conversation.
Personal Codex configuration or reusable skills may be copied separately, but
they are not required to understand this repository.

## 6. Optional local data

The repository intentionally does not contain:

- the SQLite application database;
- Windows Credential Manager secrets;
- private calendar URLs;
- imported screenshots;
- Codex credentials or account sessions;
- `node_modules`, Rust `target`, logs, or build output.

To continue with the same demo data, copy
`%APPDATA%\com.agentbar.desktop\agent-bar.sqlite3` only if needed. Do not upload
it to GitHub. Calendar connections must be authorized again because their
secrets are machine-local.

## 7. Before continuing development

Run `git status` and confirm the worktree is clean. Then ask Codex to inspect the
repository and report the fresh test result before editing. This catches toolchain
or platform differences early.
