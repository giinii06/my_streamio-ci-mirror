# Phase 01 — Deterministic, bounded two-swarm seeder

- Precondition: existing `tests/fixtures/generate.sh` generates `reference.mp4` and `alternate.mp4`; `librqbit = =9.0.1`.
- RED: test must fail if both generated torrents have the same Infohash or fixture bytes are copied instead of seeded; test asserts distinct hashes and nonempty real files.
- Design: `librqbit::Session::create_and_serve_torrent(path, CreateTorrentOptions { piece_length: Some(...), trackers: vec![], ..Default::default() })` starts and serves each fixture in a separate seed session, returns magnet and bound listener port.
- Use temp directories, real generated footage, short test deadlines. Session listener resolves to a loopback address via `listen_addr` / `announce_port`; never use public tracker.
- Positive: two distinct and valid magnets, seed session ready, no network payload data injected directly in downloader storage.
- Negative: no seed or invalid magnet fails closed.
- Precommit gate: check Rust syntax and compile via available executor; if local toolchain unavailable, remote GitHub Jobs required before `tested`/ `verified`.
- Rollback: remove new fixture test module; no runtime data migration.
- State: `not_started`. Implemented=no; Tested=no; Committed=no.
