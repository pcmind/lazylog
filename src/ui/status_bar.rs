use crate::commands::Mode;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

pub struct StatusBar;

impl StatusBar {
    pub fn render(mode: &Mode, current_line: usize, total_lines: usize, file_size: u64, filter_input: &str, is_filter_pane: bool) -> Paragraph<'static> {
        let string_hints;
        
        let (mode_str, hints) = match mode {
            Mode::Normal => {
                if is_filter_pane {
                    (" FILTER ", "[Ctrl+P] Pane  [e] Edit  [r] Regex  [b] Bookmarks  [x] Close  [X] Close Others  [0-9] Jump  [q] Quit")
                } else {
                    (" NORMAL ", "[Ctrl+P] Pane  [f] Filter  [s] Stack  [m] Mark  [x] Close  [0-9] Jump  [q] Quit")
                }
            },
            Mode::Pane => {
                if is_filter_pane {
                    (" FILTER ", "[Esc] Normal  [e] Edit  [s] Stack  [r] Regex  [b] Bookmarks  [x] Close  [0-9] Jump  [j/k] Scroll")
                } else {
                    (" PANE ", "[Esc] Normal  [n] New Filter  [s] Stack  [x] Close  [0-9] Jump  [j/k] Scroll")
                }
            },
            Mode::Tab => (" TAB ", "[Esc] Normal  [j/k] Switch Tab  [n] New Tab"),
            Mode::Filter => {
                string_hints = format!("[Esc] Cancel  [Enter] Confirm  > {}", filter_input);
                (" INPUT ", string_hints.as_str())
            }
        };

        let metrics_str = if total_lines > 0 {
            format!(" | Line {}/{} | Size: {} B ", current_line, total_lines, file_size)
        } else {
            String::new()
        };

        let mode_span = Span::styled(
            mode_str,
            Style::default()
                .bg(Color::Blue)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        );

        let hints_span = Span::styled(
            format!(" {}{}", hints, metrics_str),
            Style::default().fg(Color::White),
        );

        let empty_space = Span::styled(
            " ".repeat(200), // Quick hack to fill bg, normally use a widget that supports bg filling
            Style::default().bg(Color::DarkGray),
        );

        Paragraph::new(Line::from(vec![mode_span, hints_span, empty_space]))
            .style(Style::default().bg(Color::DarkGray).fg(Color::White))
    }
}
