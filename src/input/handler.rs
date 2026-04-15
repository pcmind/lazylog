use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::Instant;

use crate::state::action::{Action, ActionId, FilterIntent, Mode};
use crate::input::keys::{KeyCombo, KeyRegistry, LookupResult};

/// Central input state machine: translates key events into Actions based on current Mode.
pub struct CommandHandler {
    pub mode: Mode,
    pub filter_input: String,
    pub filter_intent: FilterIntent,
    pub help_selected: usize,
    pub pending_keys: Vec<KeyCombo>,
    pub pending_timeout: Instant,
    pub registry: KeyRegistry,
    // Search state
    pub search_input: String,
    pub search_query: Option<String>,
    // Help filter state
    pub help_filter: String,
}

impl CommandHandler {
    pub fn new() -> Self {
        Self {
            mode: Mode::Normal,
            filter_input: String::new(),
            filter_intent: FilterIntent::New,
            help_selected: 0,
            pending_keys: Vec::new(),
            pending_timeout: Instant::now(),
            registry: KeyRegistry::default_bindings(),
            search_input: String::new(),
            search_query: None,
            help_filter: String::new(),
        }
    }

    pub fn check_timeout(&mut self) -> Action {
        if !self.pending_keys.is_empty() && self.pending_timeout.elapsed().as_millis() > 1500 {
            self.pending_keys.clear();
        }
        Action::None
    }

    pub fn handle_key(&mut self, key: KeyEvent, current_line: usize) -> Action {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Action::Quit;
        }

        self.check_timeout();

        match self.mode {
            Mode::Normal => self.handle_normal(key, current_line),
            Mode::Filter => self.handle_filter(key),
            Mode::Search => self.handle_search(key),
            Mode::Help => self.handle_help(key),
            Mode::Visual { anchor_line } => self.handle_visual(key, anchor_line),
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
                self.mode = Mode::Visual { anchor_line: current_line };
                return Action::EnterVisual;
            }
            if let KeyCode::Char(c) = key.code {
                if c.is_ascii_digit() {
                    return Action::FocusPane(c.to_digit(10).unwrap() as usize);
                }
            }
        }

        self.pending_keys.push(KeyCombo::unshifted(&key));
        self.pending_timeout = Instant::now();

        match self.registry.lookup(&self.pending_keys) {
            LookupResult::ExactMatch(action_id) => {
                self.pending_keys.clear();
                self.execute_action(action_id, current_line)
            }
            LookupResult::PartialMatch => {
                Action::None
            }
            LookupResult::NoMatch => {
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
        self.pending_timeout = Instant::now();

        match self.registry.lookup(&self.pending_keys) {
            LookupResult::ExactMatch(action_id) => {
                self.pending_keys.clear();
                if action_id == ActionId::Yank {
                    self.mode = Mode::Normal;
                    return Action::Yank(anchor);
                } else if action_id == ActionId::ScrollDown || action_id == ActionId::ScrollUp {
                    return self.execute_action(action_id, Default::default());
                } else {
                    Action::None
                }
            }
            LookupResult::PartialMatch => Action::None,
            LookupResult::NoMatch => {
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
                Action::None
            }
            KeyCode::Enter => {
                self.mode = Mode::Normal;
                let query = self.filter_input.clone();
                self.filter_input.clear();
                let intent = self.filter_intent;
                Action::SubmitFilter(query, intent)
            }
            KeyCode::Backspace => {
                self.filter_input.pop();
                Action::None
            }
            KeyCode::Char(c) => {
                self.filter_input.push(c);
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
                Action::None
            }
            KeyCode::Enter => {
                self.mode = Mode::Normal;
                let query = self.search_input.clone();
                self.search_input.clear();
                if query.is_empty() {
                    self.search_query = None;
                    Action::ClearSearch
                } else {
                    self.search_query = Some(query.clone());
                    Action::SubmitSearch(query)
                }
            }
            KeyCode::Backspace => {
                self.search_input.pop();
                Action::None
            }
            KeyCode::Char(c) => {
                self.search_input.push(c);
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
        self.registry.all_bindings().iter().filter(|b| {
            b.description.to_lowercase().contains(&filter_lower)
            || b.label.to_lowercase().contains(&filter_lower)
            || b.display_key().to_lowercase().contains(&filter_lower)
        }).count()
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
                    self.registry.all_bindings().iter().filter(|b| {
                        b.description.to_lowercase().contains(&filter_lower)
                        || b.label.to_lowercase().contains(&filter_lower)
                        || b.display_key().to_lowercase().contains(&filter_lower)
                    }).nth(self.help_selected)
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
        }
    }
}
