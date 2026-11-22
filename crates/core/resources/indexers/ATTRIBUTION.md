# Indexer definition attribution

The five `.yml` files in this directory are copied verbatim, without modification,
for operator convenience so MEDIA_HAVEN can seed them into a local Prowlarr
instance.

- `yts.yml`, `nyaasi.yml`, `thepiratebay.yml` — from
  [Prowlarr/Indexers](https://github.com/Prowlarr/Indexers)
  (`definitions/v11/` on `master`).
- `eztv-api.yml`, `animetosho.yml` — from
  [maury91/miauflix-indexers](https://github.com/maury91/miauflix-indexers)
  (`definitions/` on `main`).

Definitions are embedded into the binary from this directory with `include_str!`;
there is only one copy of each file.
