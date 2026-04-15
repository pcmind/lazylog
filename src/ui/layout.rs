use ratatui::layout::{Constraint, Direction, Layout, Rect};

// Defines the Zellij-like main layout tree
pub struct LayoutTree;

impl LayoutTree {
    pub fn split_main(area: Rect) -> (Rect, Rect) {
        // [0] Panes Area
        // [1] Status Bar (1 line)
        let parts = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Min(1),    // Main content
                    Constraint::Length(1), // Status Bar
                ]
                .as_ref(),
            )
            .split(area);

        (parts[0], parts[1])
    }
}
