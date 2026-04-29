# lazylog

`lazylog` is a high-performance, asynchronous TUI log viewer built with Rust, Ratatui, and Tokio. It's designed to handle massive log files efficiently by indexing line offsets and performing filtering in background tasks.

## Project Overview

- **Core Technologies**: Rust, Ratatui (TUI), Tokio (Async), Crossterm (Events), Regex.
- **Architecture**:
    - **`state/`**: Manages application state. `App` contains `Tab`s, which contain multiple `Pane`s (main view and filters).
    - **`io/`**: Core log processing logic.
        - `indexer.rs`: Background task that scans the file for line offsets.
        - `reader.rs`: Asynchronous reading of specific lines/ranges using indexed offsets.
        - `filter.rs`: Background filtering tasks that populate matched line indices.
        - `query.rs`: Boolean expression parser and matcher for advanced filtering.
    - **`ui/`**: Rendering logic using Ratatui widgets.
    - **`input/`**: Keyboard event handling and action dispatching.
    - **`dispatch.rs`**: The central "brain" that receives actions and mutates state.

## Building and Running

### Development Commands

- **Build**: `cargo build`
- **Run**: `cargo run -- <path_to_log_file>`
- **Test**: `cargo test`
- **Lint**: `cargo clippy`
- **Format**: `cargo fmt`

### Production Build

```bash
cargo build --release
```

## Development Conventions

- **Error Handling**: Uses `color_eyre::Result` for application-level error reporting.
- **Concurrency**: 
    - UI thread remains responsive by offloading heavy work (indexing, filtering, complex reading) to background Tokio tasks or `spawn_blocking`.
    - Uses `Arc<RwLock<...>>` for thread-safe access to shared data like line offsets and matched indices.
- **State Mutation**: Follows an Action-Dispatch pattern. UI events are translated into `Action`s which are processed in `dispatch.rs`.
- **Surgical Reads**: Never load the whole file into memory. Always use `AsyncReader` to fetch only the visible lines.
- **Task Generations**: Background tasks use a "generation" counter to cancel themselves if a newer task (e.g., updated filter query) is spawned.

## Key Files

- `src/main.rs`: Entry point and main event loop.
- `src/state/app.rs`: Main application and tab state structures.
- `src/io/filter.rs`: Implementation of the background filtering engine.
- `src/dispatch.rs`: Action handler that implements the core business logic.
- `src/input/handler.rs`: Keyboard binding definitions and mode management.
- `src/config.rs`: Configuration loading (TOML) and custom highlighting logic.
