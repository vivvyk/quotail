//! UI layer. Reads `&AppState`, never mutates it. `render()` (added in step 4)
//! is the root that splits the frame and dispatches to the per-view modules.

pub mod layout;
pub mod theme;

pub mod bottom_bar;
pub mod chart;
pub mod detail;
pub mod help;
pub mod overview;
pub mod settings;
pub mod table;
