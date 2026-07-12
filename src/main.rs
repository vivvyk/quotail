//! Quotail entry point.
//!
//! Arg parsing (`quotail` TUI vs `--mcp` vs the JSON CLI) lands here in later
//! steps. The module tree lives in the library crate (`src/lib.rs`) so every
//! binary target shares it; this bin just drives it.

fn main() {
    // Real entry (terminal setup, event loop) arrives in step 3.
    println!("quotail — scaffolding build");
}
