# Ghost runtime web

SvelteKit 2 / Svelte 5 system shell for `ghost-fl-runtime`. Bun owns dependency and frontend tooling; Rust remains the parent process, lifecycle authority, HTTP/WebSocket server, and production asset host.

## Recreate the workspace

The workspace was created with the current Svelte CLI and all requested integrations selected non-interactively:

```sh
bun x --bun sv@0.17.0 create web/runtime --template minimal --types ts --add prettier eslint "tailwindcss=plugins:none" playwright "vitest=usages:unit" "sveltekit-adapter=adapter:static" --install bun
```

## Developing

Run from `web/`:

```sh
bun install
bun run check
bun run test
bun run build
```

Run from the repository root when Rust should orchestrate the web toolchain:

```powershell
cargo xtask bindings
cargo xtask web-check
cargo xtask web-build
cargo xtask web
```

Development uses Vite and proxies `/api` plus WebSocket upgrades to the default runtime address:

```powershell
bun run --cwd web/runtime dev
```

Open `http://127.0.0.1:5173/?mock=1` to exercise the UI without FL Studio. Production uses SvelteKit's static fallback build; `rust-embed` serves those files from the Rust binary. Registered applications render under `/apps/[appId]` and retain their own domain state and models.
