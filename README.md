# Audiobookshelf for lemonfiber

**Listening to audiobooks, with progress synced.**
Audiobookshelf is one of the parts the stack lemonfiber ships is built from.
Without it: No audiobook player.

## How lemonfiber carries it

The default bundle in
[lemonfiber-plugins](https://github.com/lemonfiber/lemonfiber-plugins) pins this
repository by a revision and the digests of its manifest and its tree, and
lemonfiber compiles that tree in.
This repository holds the adapter that speaks lemonfiber's `owner.claim` and
`media.serve` contracts in front of Audiobookshelf: a Rust crate built on the core's
adapter kit, into the image `ghcr.io/lemonfiber/audiobookshelf-adapter`, published
from every commit on `main` and from each tag the release train cuts. It makes
Audiobookshelf's root account for the one lemonfiber mints before anybody else can,
reads what its libraries hold and what is playing, scans its libraries again, mints
and revokes its API keys and sets the origins a browser may read it from.
`plugin.toml` does not name the adapter yet, so lemonfiber assembles Audiobookshelf
into the stack it ships as a stack service, and reaches it through the in-process
adapter its `api` names.

## What is here

| Path | What it is |
| --- | --- |
| `plugin.toml` | The part's manifest |
| `recordings/audiobookshelf/` | The responses its claims are proved against |
| `src/` | The adapter |
| `Dockerfile` | The adapter's image |
| `targets.toml` | The lemonfiber its checks run against, and the core the adapter is built on |
| `proofs.json` | What those checks said, committed |

## Checks

The plugin verdict is one job of spec's
[`first-party-plugin`](https://github.com/lemonfiber/spec/blob/main/.github/workflows/first-party-plugin.yml)
workflow: the manifest as the lemonfiber `targets.toml` names reads it, every
claim against its recording, and every pinned digest. The adapter is held to its
format, its lints, its tests, full line coverage and its dependency policy, its
image runs as lemonfiber runs it, and `recordings` takes every recording again
against the pinned Audiobookshelf. `just ci` runs the gates that need nothing but
this tree.

## Licence

The code and data in this repository are under the licence in `LICENSE`.
Audiobookshelf itself is under GPL-3.0-only and is not distributed here: this
repository names an image, it does not contain one.
