# Contributing to deck

Thanks for your interest in contributing!

## Getting Started

```bash
git clone https://github.com/thijsvos/deck.git
cd deck
cargo build
cargo run -- examples/demo.md
```

## Making Changes

1. Fork the repository
2. Create a branch (`git checkout -b feature/your-feature`)
3. Make your changes
4. Run `cargo clippy` and fix any warnings
5. Run `cargo fmt` to format your code
6. Test with `cargo run -- examples/demo.md`
7. Commit and push your branch
8. Open a pull request

## Adding a New Background

Backgrounds live in `src/background.rs`. To add one:

1. Add a variant to the `BackgroundKind` enum
2. Add the variant to the `compute_cell` match (for per-cell) or `apply_background` match (for scatter-based)
3. Write your rendering function - it receives `(x, y, width, height, time)` and returns `(char, brightness)`
4. Add the name to the `extract_background` parser in `src/parse.rs`
5. Add it to the backgrounds table in `README.md`

Per-cell backgrounds should be O(1) per cell. If your math needs to pre-compute points (like Lissajous or Orbit), use the scatter approach instead.

## Code Style

- Run `cargo fmt` before committing
- Run `cargo clippy` and address warnings
- Keep the binary small - avoid adding dependencies unless truly necessary

## Reporting Issues

Use the [issue templates](https://github.com/thijsvos/deck/issues/new/choose) to report bugs or request features.
