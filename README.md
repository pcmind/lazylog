# lazylog

`lazylog` is a blazing fast, asynchronous terminal log viewer designed for high-performance log analysis. Built with Rust and Ratatui, it combines the speed of native tools like `grep` with an interactive, multi-pane TUI workflow.

![Lazylog TUI](https://img.shields.io/badge/TUI-Ratatui-blue)
![Language-Rust](https://img.shields.io/badge/Language-Rust-orange)

## Features

- **⚡ Blazing Fast Asynchronous Engine**: Built on Tokio, `lazylog` indexes and filters millions of lines in the background without locking the UI.
- **🪟 Multi-Pane Workflow**: Open multiple filter panes to view different parts of your logs simultaneously.
- **🔗 Chained/Chained Filtering**: Create sub-filters from existing results to drill down into complex logs.
- **🕵️ Professional Search**: Global search with highlighting and fast navigation.
- **✨ Custom Highlighting**: Define persistent color highlighters for specific patterns (e.g., `ERROR`, `DEBUG`) in a simple TOML configuration.
- **🎣 Real-time Tail**: Follow logs in real-time with an intelligent `Follow` mode that preserves your scroll position until new data arrives.
- **📍 Bookmarking**: Mark important lines and view them interleaved with your filtered results.
- **📋 Clipboard Support**: Select and yank lines using Visual mode for easy sharing.

## Installation

### From Source

Ensure you have Rust and Cargo installed, then:

```bash
git clone https://github.com/your-username/lazylog.git
cd lazylog
cargo build --release
cp target/release/lazylog /usr/local/bin/
```

## Usage

Simply pass the log file as an argument:

```bash
lazylog app.log
```

## Keyboard Shortcuts

### Global / Navigation
| Key | Action |
| :--- | :--- |
| `q` | Quit |
| `j` / `k` / `↑` / `↓` | Scroll line by line |
| `Ctrl-d` / `Ctrl-u` | Page Down / Up |
| `g g` / `G` | Go to Top / Bottom |
| `h` / `l` / `←` / `→` | Horizontal Scroll |
| `Tab` / `Shift-Tab` | Focus Next / Previous Pane |
| `?` | Show Help Menu |

### Filtering (Active on Filter Panes)
Press `e` to enter the **Edit/Params** sub-menu:
| Key | Action |
| :--- | :--- |
| `f` | Create New Filter |
| `e e` | Edit current filter query |
| `e r` | Toggle **Regex** mode |
| `e n` | Toggle **Negate** filter (exclude matches) |
| `e c` | Toggle **Case Sensitive** matching |
| `e b` | Toggle **Bookmarks** visibility in this pane |
| `p` | **Pin** filter (keeps it visible when inactive) |
| `x` / `X` | Close current / all other filter panes |

### Search & Selection
| Key | Action |
| :--- | :--- |
| `/` | Begin global search |
| `n` / `N` | Jump to Next / Previous search result |
| `F` | Toggle **Follow** (tail -f) mode |
| `m` | **Bookmark** / Unmark current line |
| `v` | Enter **Visual Mode** for line selection |
| `y` | **Yank** selected lines to clipboard (Visual mode) |

## Configuration

`lazylog` can be configured via a TOML file located at `~/.config/lazylog/config.toml`.

### Line Highlighting
You can define custom colors for log levels or specific patterns:

```toml
[[highlighter]]
pattern = "ERROR"
is_regex = false
fg = "Red"
bg = "Black"

[[highlighter]]
pattern = "TRACE|DEBUG"
is_regex = true
fg = "DarkGray"
```

Supporting colors: `Red`, `Green`, `Yellow`, `Blue`, `Magenta`, `Cyan`, `Gray`, `DarkGray`, `LightRed`, etc. (standard TUI colors).

## Performance Notes

`lazylog` uses a custom indexing strategy that only loads necessary line offsets into memory, allowing it to handle files much larger than the available RAM. Filtering tasks are spawned as background workers that populate results progressively, ensuring the interface remains snappy even during heavy scans.

