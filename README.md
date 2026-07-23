# rehash

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![CI](https://img.shields.io/badge/CI-passing-brightgreen.svg)](https://github.com/Emran-goat/rehash/actions)

A build cache daemon. It hashes your source files, build config, toolchain, and environment variables to produce a cache key. If the key exists in the cache, the build output gets restored directly instead of running the build again.

## How it works

![System Context](docs/architecture/images/c4-context--0.svg)

rehash sits between you and your build tool. When you run a build through it, the CLI hashes your inputs and checks the daemon for a match. On a cache hit, it restores the previous output. On a miss, it runs the real build and stores the result for next time.

### Cache hit vs miss flow

![Build Flow](docs/architecture/images/c4-dynamic-build--0.svg)

## Installation

```bash
cargo install rehash
```

Or build from source:

```bash
git clone https://github.com/Emran-goat/rehash.git
cd rehash
cargo build --release
```

## Usage

Run a build through the cache:

```bash
rehash build cargo build
rehash build make
rehash build npm run build
```

Start the daemon:

```bash
rehash-daemon
```

Show cache stats:

```bash
rehash stats
```

Clear the cache:

```bash
rehash clear
```

## Architecture

rehash is split into three crates:

![Containers](docs/architecture/images/c4-containers--0.svg)

**rehash-cli** wraps your build command, hashes inputs, and talks to the daemon over IPC.

**rehash-daemon** runs in the background, manages the cache store, watches your source files for changes, and invalidates stale entries.

**rehash-core** is the library that does the actual work: hashing (blake3), caching (sled), metadata storage (SQLite), and compression (zstd).

![Core Components](docs/architecture/images/c4-components--0.svg)

### Deployment

![Deployment](docs/architecture/images/c4-deployment--0.svg)

The daemon stores cached artifacts under `~/.cache/rehash/` with metadata in a SQLite database.

### Build lifecycle

![Build Lifecycle](docs/architecture/images/build-lifecycle--0.svg)

## Project structure

```
rehash/
  Cargo.toml          # workspace root
  crates/
    rehash-core/      # hashing, cache engine, storage, compression
    rehash-cli/       # CLI tool (rehash build, stats, clear)
    rehash-daemon/    # background daemon (watcher, IPC server)
  docs/
    architecture/     # C4 architecture diagrams and docs
```

## License

MIT
