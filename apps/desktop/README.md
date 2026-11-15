# apps/desktop

The single user-facing surface: React + TypeScript UI inside a Tauri 2 shell.

**Status: not created yet.** `roadmap.md` P1-01 creates this directory's
contents. This README defines what belongs here so the structure is not invented
ad hoc later.

## Intended layout

```
apps/desktop/
├── package.json
├── vite.config.ts
├── tsconfig.json
├── index.html
├── src/
│   ├── app/          App.tsx, routes.tsx, providers
│   ├── components/   navigation, media, player, cards, shelves, dialogs, feedback
│   ├── pages/        Home, Movies, Shows, Library, Downloads, Search, Player, Settings, Setup
│   ├── hooks/        FocusManager, directional navigation, typed command hooks
│   ├── state/        client state only — never persisted business state
│   ├── services/     typed wrappers around Tauri commands (no logic)
│   ├── styles/       tokens.css, global.css, motion.css
│   ├── types/        DTOs mirroring the Rust boundary
│   └── test/
└── src-tauri/
    ├── Cargo.toml
    ├── tauri.conf.json
    ├── capabilities/
    ├── icons/
    └── src/          main.rs, lib.rs, commands/
```

## Rules for this directory

- **Presentation only.** No filesystem deletion, SQL, policy, or Docker daemon access. The Rust backend in the required `core` container decides; this layer displays and collects intent through named Tauri commands.
- **Tokens only.** No colour or spacing literal outside `src/styles/`.
- **Typed boundary.** Every product operation goes through `src/services/` and named Tauri commands. The frontend must never issue Docker/runtime commands or receive a generic Docker/backend passthrough.
- **Every state designed.** Empty, loading, offline, missing-artwork and error
  states are part of the component, not an afterthought.
- **Keyboard first.** Every interactive element is reachable and operable with
  arrows + Enter, and has an accessible name.
- **No real content.** No hard-coded titles, poster URLs or file paths in
  production UI. Mock data lives in tests and fixtures.

## Related documents

- `docs/design/design-system.md` — tokens, type, motion, focus
- `docs/design/navigation-spec.md` — focus model and traversal rules
- `Development.md` §7 — the full UI contract
