# `demo-site`

Build output, not source. An orphan branch: it shares no history with `main`
and must never be merged into it.

It exists because the libcosmic branch cannot be built on CI. `Cargo.toml`
there points `libcosmic` at a path — a working copy carrying patches to
libcosmic and to its vendored `iced` submodule — so `pages.yml`'s
`nix build .#harken-demo` has nothing to fetch. The committable answer is a
fork of pop-os/libcosmic with those patches applied, pinned by git rev the
way petros is; until that exists, this branch carries the wasm instead.

`.github/workflows/pages-demo.yml` publishes it, on `workflow_dispatch` only.

Delete the branch and the workflow together the day the fork lands.
