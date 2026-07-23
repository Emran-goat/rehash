# Architecture Documentation

This directory contains the architecture documentation for rehash, organized using the C4 model.

## Diagrams

| Level | Diagram | Description |
|-------|---------|-------------|
| Context | [System Context](c4-context.md) | rehash and its external actors (developers, CI runners, build tools) |
| Container | [Containers](c4-containers.md) | The three crates: CLI, daemon, and core library |
| Component | [Components](c4-components.md) | Internal structure of rehash-core: hasher, cache engine, metadata store, compressor |
| Deployment | [Deployment](c4-deployment.md) | How rehash runs on a developer machine |
| Dynamic | [Build Flow](c4-dynamic-build.md) | Cache hit vs miss sequence |
| Flowchart | [Build Lifecycle](build-lifecycle.md) | Decision tree for the build process |

Each diagram is available as both inline Mermaid code and SVG image.
