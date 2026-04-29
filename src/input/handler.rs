use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::input::keys::{KeyCombo, KeyRegistry, LookupResult};
use crate::state::action::{Action, ActionId, FilterIntent, Mode};

/// Central input state machine: translates key events into Actions based on current Mode.
pub struct CommandHandler {
    pub mode: Mode,
    pub filter_input: String,
    pub filter_cursor: usize,
    pub filter_intent: FilterIntent,
    pub help_selected: usize,
    pub pending_keys: Vec<KeyCombo>,
    pub registry: KeyRegistry,
    pub filter_history: Vec<String>,
    pub filter_history_idx: Option<usize>,
    // Search state
    pub search_input: String,
    pub search_cursor: usize,
    pub search_query: Option<String>,
    pub search_history: Vec<String>,
    pub search_history_idx: Option<usize>,
    // Help filter state
    pub help_filter: String,
    pub detail_text: Option<String>,
}

impl CommandHandler {
    pub fn new() -> Self {
        Self {
            mode: Mode::Normal,
            filter_input: String::new(),
            filter_cursor: 0,
            filter_intent: FilterIntent::New,
            help_selected: 0,
            pending_keys: Vec::new(),
            registry: KeyRegistry::default_bindings(),
            filter_history: Vec::new(),
            filter_history_idx: None,
            search_input: String::new(),
            search_cursor: 0,
            search_query: None,
            search_history: Vec::new(),
            search_history_idx: None,
            help_filter: String::new(),
            detail_text: None,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, current_line: usize) -> Action {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Action::Quit;
        }

        match self.mode {
            Mode::Normal => self.handle_normal(key, current_line),
            Mode::Filter => self.handle_filter(key),
            Mode::Search => self.handle_search(key),
            Mode::Help => self.handle_help(key),
            Mode::Visual { anchor_line } => self.handle_visual(key, anchor_line),
            Mode::LineDetail => self.handle_line_detail(key),
        }
    }

    fn handle_normal(&mut self, key: KeyEvent, current_line: usize) -> Action {
        // Esc in Normal mode clears active search
        if key.code == KeyCode::Esc && self.search_query.is_some() {
            self.search_query = None;
            return Action::ClearSearch;
        }

        if self.pending_keys.is_empty() {
            if let KeyCode::Char('v') = key.code {
                self.mode = Mode::Visual {
                    anchor_line: current_line,
                };
                return Action::EnterVisual;
            }
            if let KeyCode::Char(c) = key.code
                && c.is_ascii_digit()
            {
                return Action::FocusPane(c.to_digit(10).unwrap() as usize);
            }
        }

        self.pending_keys.push(KeyCombo::unshifted(&key));

        match self.registry.lookup(&self.pending_keys) {
            LookupResult::Exact(action_id) => {
                self.pending_keys.clear();
                self.execute_action(action_id, current_line)
            }
            LookupResult::Partial => Action::None,
            LookupResult::None => {
                self.pending_keys.clear();
                Action::None
            }
        }
    }

    fn handle_visual(&mut self, key: KeyEvent, anchor: usize) -> Action {
        if let KeyCode::Esc | KeyCode::Char('v') = key.code {
            self.mode = Mode::Normal;
            self.pending_keys.clear();
            return Action::None;
        }

        self.pending_keys.push(KeyCombo::unshifted(&key));

        match self.registry.lookup(&self.pending_keys) {
            LookupResult::Exact(action_id) => {
                self.pending_keys.clear();
                if action_id == ActionId::Yank {
                    self.mode = Mode::Normal;
                    Action::Yank(anchor)
                } else if action_id == ActionId::ScrollDown || action_id == ActionId::ScrollUp {
                    self.execute_action(action_id, Default::default())
                } else {
                    Action::None
                }
            }
            LookupResult::Partial => Action::None,
            LookupResult::None => {
                self.pending_keys.clear();
                Action::None
            }
        }
    }

    fn handle_filter(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.filter_input.clear();
                self.filter_cursor = 0;
                self.filter_history_idx = None;
                Action::None
            }
            KeyCode::Enter => {
                self.mode = Mode::Normal;
                let query = self.filter_input.clone();
                self.filter_input.clear();
                self.filter_cursor = 0;

                if !query.is_empty() {
                    if let Some(pos) = self.filter_history.iter().position(|x| x == &query) {
                        self.filter_history.remove(pos);
                    }
                    self.filter_history.push(query.clone());
                    if self.filter_history.len() > 50 {
                        self.filter_history.remove(0);
                    }
                }
                self.filter_history_idx = None;

                let intent = self.filter_intent;
                Action::SubmitFilter(query, intent)
            }
            KeyCode::Backspace => {
                if self.filter_cursor > 0 {
                    let byte_idx = self
                        .filter_input
                        .char_indices()
                        .nth(self.filter_cursor - 1)
                        .map(|(i, _)| i)
                        .unwrap();
                    self.filter_input.remove(byte_idx);
                    self.filter_cursor -= 1;
                }
                Action::None
            }
            KeyCode::Delete => {
                if self.filter_cursor < self.filter_input.chars().count() {
                    let byte_idx = self
                        .filter_input
                        .char_indices()
                        .nth(self.filter_cursor)
                        .map(|(i, _)| i)
                        .unwrap();
                    self.filter_input.remove(byte_idx);
                }
                Action::None
            }
            KeyCode::Left => {
                self.filter_cursor = self.filter_cursor.saturating_sub(1);
                Action::None
            }
            KeyCode::Right => {
                if self.filter_cursor < self.filter_input.chars().count() {
                    self.filter_cursor += 1;
                }
                Action::None
            }
            KeyCode::Home => {
                self.filter_cursor = 0;
                Action::None
            }
            KeyCode::End => {
                self.filter_cursor = self.filter_input.chars().count();
                Action::None
            }
            KeyCode::Char(c) => {
                let byte_idx = self
                    .filter_input
                    .char_indices()
                    .nth(self.filter_cursor)
                    .map(|(i, _)| i)
                    .unwrap_or(self.filter_input.len());
                self.filter_input.insert(byte_idx, c);
                self.filter_cursor += 1;
                Action::None
            }
            KeyCode::Up => {
                if !self.filter_history.is_empty() {
                    let new_idx = match self.filter_history_idx {
                        None => self.filter_history.len().saturating_sub(1),
                        Some(i) => i.saturating_sub(1),
                    };
                    self.filter_history_idx = Some(new_idx);
                    self.filter_input = self.filter_history[new_idx].clone();
                    self.filter_cursor = self.filter_input.chars().count();
                }
                Action::None
            }
            KeyCode::Down => {
                if let Some(i) = self.filter_history_idx {
                    if i + 1 < self.filter_history.len() {
                        let new_idx = i + 1;
                        self.filter_history_idx = Some(new_idx);
                        self.filter_input = self.filter_history[new_idx].clone();
                    } else {
                        self.filter_history_idx = None;
                        self.filter_input.clear();
                    }
                    self.filter_cursor = self.filter_input.chars().count();
                }
                Action::None
            }
            _ => Action::None,
        }
    }

    fn handle_search(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.search_input.clear();
                self.search_cursor = 0;
                self.search_history_idx = None;
                Action::None
            }
            KeyCode::Enter => {
                self.mode = Mode::Normal;
                let query = self.search_input.clone();
                self.search_input.clear();
                self.search_cursor = 0;

                if !query.is_empty() {
                    if let Some(pos) = self.search_history.iter().position(|x| x == &query) {
                        self.search_history.remove(pos);
                    }
                    self.search_history.push(query.clone());
                    if self.search_history.len() > 50 {
                        self.search_history.remove(0);
                    }

                    self.search_query = Some(query.clone());
                    self.search_history_idx = None;
                    Action::SubmitSearch(query)
                } else {
                    self.search_query = None;
                    self.search_history_idx = None;
                    Action::ClearSearch
                }
            }
            KeyCode::Backspace => {
                if self.search_cursor > 0 {
                    let byte_idx = self
                        .search_input
                        .char_indices()
                        .nth(self.search_cursor - 1)
                        .map(|(i, _)| i)
                        .unwrap();
                    self.search_input.remove(byte_idx);
                    self.search_cursor -= 1;
                }
                Action::None
            }
            KeyCode::Delete => {
                if self.search_cursor < self.search_input.chars().count() {
                    let byte_idx = self
                        .search_input
                        .char_indices()
                        .nth(self.search_cursor)
                        .map(|(i, _)| i)
                        .unwrap();
                    self.search_input.remove(byte_idx);
                }
                Action::None
            }
            KeyCode::Left => {
                self.search_cursor = self.search_cursor.saturating_sub(1);
                Action::None
            }
            KeyCode::Right => {
                if self.search_cursor < self.search_input.chars().count() {
                    self.search_cursor += 1;
                }
                Action::None
            }
            KeyCode::Home => {
                self.search_cursor = 0;
                Action::None
            }
            KeyCode::End => {
                self.search_cursor = self.search_input.chars().count();
                Action::None
            }
            KeyCode::Char(c) => {
                let byte_idx = self
                    .search_input
                    .char_indices()
                    .nth(self.search_cursor)
                    .map(|(i, _)| i)
                    .unwrap_or(self.search_input.len());
                self.search_input.insert(byte_idx, c);
                self.search_cursor += 1;
                Action::None
            }
            KeyCode::Up => {
                if !self.search_history.is_empty() {
                    let new_idx = match self.search_history_idx {
                        None => self.search_history.len().saturating_sub(1),
                        Some(i) => i.saturating_sub(1),
                    };
                    self.search_history_idx = Some(new_idx);
                    self.search_input = self.search_history[new_idx].clone();
                    self.search_cursor = self.search_input.chars().count();
                }
                Action::None
            }
            KeyCode::Down => {
                if let Some(i) = self.search_history_idx {
                    if i + 1 < self.search_history.len() {
                        let new_idx = i + 1;
                        self.search_history_idx = Some(new_idx);
                        self.search_input = self.search_history[new_idx].clone();
                    } else {
                        self.search_history_idx = None;
                        self.search_input.clear();
                    }
                    self.search_cursor = self.search_input.chars().count();
                }
                Action::None
            }
            _ => Action::None,
        }
    }

    /// Filtered bindings count for help navigation bounds
    fn help_filtered_count(&self) -> usize {
        if self.help_filter.is_empty() {
            return self.registry.all_bindings().len();
        }
        let filter_lower = self.help_filter.to_lowercase();
        self.registry
            .all_bindings()
            .iter()
            .filter(|b| {
                b.description.to_lowercase().contains(&filter_lower)
                    || b.label.to_lowercase().contains(&filter_lower)
                    || b.display_key().to_lowercase().contains(&filter_lower)
            })
            .count()
    }

    fn handle_help(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.help_filter.clear();
                self.help_selected = 0;
                Action::None
            }
            KeyCode::Down => {
                let count = self.help_filtered_count();
                if count > 0 && self.help_selected + 1 < count {
                    self.help_selected += 1;
                }
                Action::None
            }
            KeyCode::Up => {
                self.help_selected = self.help_selected.saturating_sub(1);
                Action::None
            }
            KeyCode::Backspace => {
                self.help_filter.pop();
                self.help_selected = 0;
                Action::None
            }
            KeyCode::Enter => {
                let filter_lower = self.help_filter.to_lowercase();
                let binding = if self.help_filter.is_empty() {
                    self.registry.all_bindings().get(self.help_selected)
                } else {
                    self.registry
                        .all_bindings()
                        .iter()
                        .filter(|b| {
                            b.description.to_lowercase().contains(&filter_lower)
                                || b.label.to_lowercase().contains(&filter_lower)
                                || b.display_key().to_lowercase().contains(&filter_lower)
                        })
                        .nth(self.help_selected)
                };
                if let Some(b) = binding {
                    let action_id = b.action;
                    self.mode = Mode::Normal;
                    self.help_filter.clear();
                    self.help_selected = 0;
                    self.execute_action(action_id, 0)
                } else {
                    Action::None
                }
            }
            KeyCode::Char(c) => {
                self.help_filter.push(c);
                self.help_selected = 0;
                Action::None
            }
            _ => Action::None,
        }
    }

    fn handle_line_detail(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                self.mode = Mode::Normal;
                Action::None
            }
            _ => Action::None,
        }
    }

    fn execute_action(&mut self, action_id: ActionId, _current_line: usize) -> Action {
        match action_id {
            ActionId::Quit => Action::Quit,
            ActionId::ScrollDown => Action::ScrollDown,
            ActionId::ScrollUp => Action::ScrollUp,
            ActionId::NextPane => Action::NextPane,
            ActionId::PrevPane => Action::PrevPane,
            ActionId::NewFilter => {
                self.filter_intent = FilterIntent::New;
                self.mode = Mode::Filter;
                Action::None
            }

            ActionId::EditFilter => {
                self.filter_intent = FilterIntent::Edit;
                Action::EditFilter
            }
            ActionId::ToggleRegex => Action::ToggleRegex,
            ActionId::ToggleNegate => Action::ToggleNegate,
            ActionId::ToggleInterleave => Action::ToggleInterleave,
            ActionId::ToggleCaseSensitive => Action::ToggleCaseSensitive,
            ActionId::ToggleBookmark => Action::ToggleBookmark,
            ActionId::ClosePane => Action::ClosePane,
            ActionId::CloseOtherPanes => Action::CloseOtherPanes,
            ActionId::ShowHelp => {
                self.mode = Mode::Help;
                self.help_selected = 0;
                Action::ShowHelp
            }
            ActionId::Yank => Action::None,
            ActionId::GotoTop => Action::GotoTop,
            ActionId::GotoBottom => Action::GotoBottom,
            ActionId::HalfPageDown => Action::HalfPageDown,
            ActionId::HalfPageUp => Action::HalfPageUp,
            ActionId::PageDown => Action::PageDown,
            ActionId::PageUp => Action::PageUp,
            ActionId::ScrollLeft => Action::ScrollLeft,
            ActionId::ScrollRight => Action::ScrollRight,
            ActionId::BeginSearch => {
                self.search_input.clear();
                self.mode = Mode::Search;
                Action::BeginSearch
            }
            ActionId::NextSearchResult => Action::NextSearchResult,
            ActionId::PrevSearchResult => Action::PrevSearchResult,
            ActionId::ToggleFollow => Action::ToggleFollow,
            ActionId::TogglePinFilter => Action::TogglePinFilter,
            ActionId::ToggleBoolean => Action::ToggleBoolean,
            ActionId::ShowLineDetail => Action::ShowLineDetail,
        }
    }
}
