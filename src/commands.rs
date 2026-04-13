use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Filter,
    Search,
    Help,
    Visual { anchor_line: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionId {
    Quit, ScrollDown, ScrollUp,
    NextPane, PrevPane,
    NewFilter, StackFilter, EditFilter,
    ToggleRegex, ToggleNegate, ToggleInterleave, ToggleBookmark,
    ClosePane, CloseOtherPanes, ShowHelp,
    Yank,
    // Navigation
    GotoTop, GotoBottom, HalfPageDown, HalfPageUp, PageDown, PageUp,
    // Horizontal
    ScrollLeft, ScrollRight,
    // Search
    BeginSearch, NextSearchResult, PrevSearchResult,
    // Follow
    ToggleFollow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum BindingContext { Always, FilterPane, MainPane, VisualMode }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterIntent {
    New,
    Stack,
    Edit,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KeyCombo {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyCombo {
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }

    pub fn unshifted(key: &KeyEvent) -> Self {
        let code = match key.code {
            KeyCode::Char(c) if c.is_ascii_uppercase() => KeyCode::Char(c.to_ascii_lowercase()),
            c => c,
        };
        
        let modifiers = if let KeyCode::Char(c) = key.code {
            if c.is_ascii_uppercase() && !key.modifiers.contains(KeyModifiers::SHIFT) {
                key.modifiers | KeyModifiers::SHIFT
            } else {
                key.modifiers
            }
        } else {
            key.modifiers
        };
        Self { code, modifiers }
    }

    pub fn display_key(&self) -> String {
        let mut key_str = String::new();
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            key_str.push_str("C-");
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            key_str.push_str("A-");
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) && !matches!(self.code, KeyCode::Char(_)) {
            key_str.push_str("S-");
        }
        
        match self.code {
            KeyCode::Char(c) => {
                if self.modifiers.contains(KeyModifiers::SHIFT) {
                    key_str.push(c.to_ascii_uppercase());
                } else {
                    key_str.push(c);
                }
            }
            KeyCode::Backspace => key_str.push_str("Bksp"),
            KeyCode::Enter => key_str.push_str("Enter"),
            KeyCode::Left => key_str.push_str("Left"),
            KeyCode::Right => key_str.push_str("Right"),
            KeyCode::Up => key_str.push_str("Up"),
            KeyCode::Down => key_str.push_str("Down"),
            KeyCode::Home => key_str.push_str("Home"),
            KeyCode::End => key_str.push_str("End"),
            KeyCode::PageUp => key_str.push_str("PgUp"),
            KeyCode::PageDown => key_str.push_str("PgDn"),
            KeyCode::Tab => key_str.push_str("Tab"),
            KeyCode::BackTab => key_str.push_str("S-Tab"),
            KeyCode::Delete => key_str.push_str("Del"),
            KeyCode::Insert => key_str.push_str("Ins"),
            KeyCode::F(n) => key_str.push_str(&format!("F{}", n)),
            KeyCode::Esc => key_str.push_str("Esc"),
            _ => key_str.push_str("?"),
        }
        key_str
    }
}

#[derive(Clone)]
pub struct KeyBinding {
    pub sequence: Vec<KeyCombo>,
    pub action: ActionId,
    pub label: &'static str,
    pub description: &'static str,
    pub context: BindingContext,
}

impl KeyBinding {
    pub fn display_key(&self) -> String {
        self.sequence.iter().map(|c| c.display_key()).collect::<Vec<_>>().join(" ")
    }
}

pub struct KeyRegistry {
    pub bindings: Vec<KeyBinding>,
}

impl KeyRegistry {
    pub fn default_bindings() -> Self {
        let mut bindings = Vec::new();
        
        let push1 = |b: &mut Vec<KeyBinding>, code, modifiers, action, label, description, context| {
            b.push(KeyBinding { 
                sequence: vec![KeyCombo::new(code, modifiers)], 
                action, label, description, context 
            });
        };

        let push2 = |b: &mut Vec<KeyBinding>, c1, c2, action, label, description, context| {
            b.push(KeyBinding {
                sequence: vec![KeyCombo::new(KeyCode::Char(c1), KeyModifiers::empty()), KeyCombo::new(KeyCode::Char(c2), KeyModifiers::empty())],
                action, label, description, context
            });
        };
        
        // Core navigation
        push1(&mut bindings, KeyCode::Char('q'), KeyModifiers::empty(), ActionId::Quit, "Quit", "Quit lazylog", BindingContext::Always);
        push1(&mut bindings, KeyCode::Char('j'), KeyModifiers::empty(), ActionId::ScrollDown, "Down", "Scroll down", BindingContext::Always);
        push1(&mut bindings, KeyCode::Down, KeyModifiers::empty(), ActionId::ScrollDown, "Down", "Scroll down", BindingContext::Always);
        push1(&mut bindings, KeyCode::Char('k'), KeyModifiers::empty(), ActionId::ScrollUp, "Up", "Scroll up", BindingContext::Always);
        push1(&mut bindings, KeyCode::Up, KeyModifiers::empty(), ActionId::ScrollUp, "Up", "Scroll up", BindingContext::Always);

        // Page / half-page navigation
        push1(&mut bindings, KeyCode::Char('d'), KeyModifiers::CONTROL, ActionId::HalfPageDown, "½PgDn", "Half page down", BindingContext::Always);
        push1(&mut bindings, KeyCode::Char('u'), KeyModifiers::CONTROL, ActionId::HalfPageUp, "½PgUp", "Half page up", BindingContext::Always);
        push1(&mut bindings, KeyCode::PageDown, KeyModifiers::empty(), ActionId::PageDown, "PgDn", "Page down", BindingContext::Always);
        push1(&mut bindings, KeyCode::PageUp, KeyModifiers::empty(), ActionId::PageUp, "PgUp", "Page up", BindingContext::Always);

        // Goto top/bottom
        push1(&mut bindings, KeyCode::Char('g'), KeyModifiers::SHIFT, ActionId::GotoBottom, "Bottom", "Go to last line", BindingContext::Always);
        push2(&mut bindings, 'g', 'g', ActionId::GotoTop, "Top", "Go to first line", BindingContext::Always);

        // Horizontal scroll
        push1(&mut bindings, KeyCode::Char('h'), KeyModifiers::empty(), ActionId::ScrollLeft, "Left", "Scroll left", BindingContext::Always);
        push1(&mut bindings, KeyCode::Left, KeyModifiers::empty(), ActionId::ScrollLeft, "Left", "Scroll left", BindingContext::Always);
        push1(&mut bindings, KeyCode::Char('l'), KeyModifiers::empty(), ActionId::ScrollRight, "Right", "Scroll right", BindingContext::Always);
        push1(&mut bindings, KeyCode::Right, KeyModifiers::empty(), ActionId::ScrollRight, "Right", "Scroll right", BindingContext::Always);

        // Pane navigation
        push1(&mut bindings, KeyCode::Tab, KeyModifiers::empty(), ActionId::NextPane, "NextPane", "Focus next pane", BindingContext::Always);
        push1(&mut bindings, KeyCode::BackTab, KeyModifiers::SHIFT, ActionId::PrevPane, "PrevPane", "Focus previous pane", BindingContext::Always);

        // Filter
        push1(&mut bindings, KeyCode::Char('f'), KeyModifiers::empty(), ActionId::NewFilter, "Filter", "Create a new filter", BindingContext::Always);
        push1(&mut bindings, KeyCode::Char('f'), KeyModifiers::CONTROL, ActionId::NewFilter, "Filter", "Create a new filter", BindingContext::Always);
        push1(&mut bindings, KeyCode::Char('s'), KeyModifiers::empty(), ActionId::StackFilter, "Stack", "Stack filter on current view", BindingContext::Always);
        push1(&mut bindings, KeyCode::Char('e'), KeyModifiers::empty(), ActionId::EditFilter, "Edit", "Edit the current filter", BindingContext::FilterPane);
        push1(&mut bindings, KeyCode::Char('m'), KeyModifiers::empty(), ActionId::ToggleBookmark, "Mark", "Mark/Unmark selected line", BindingContext::Always);
        push1(&mut bindings, KeyCode::Char('x'), KeyModifiers::empty(), ActionId::ClosePane, "Close", "Close current pane", BindingContext::FilterPane);
        push1(&mut bindings, KeyCode::Char('x'), KeyModifiers::SHIFT, ActionId::CloseOtherPanes, "Close Other", "Close all other panes", BindingContext::Always);
        push1(&mut bindings, KeyCode::Char('?'), KeyModifiers::empty(), ActionId::ShowHelp, "Help", "Show keybindings menu", BindingContext::Always);

        // Search
        push1(&mut bindings, KeyCode::Char('/'), KeyModifiers::empty(), ActionId::BeginSearch, "Search", "Search in current view", BindingContext::Always);
        push1(&mut bindings, KeyCode::Char('n'), KeyModifiers::empty(), ActionId::NextSearchResult, "Next", "Next search result", BindingContext::Always);
        push1(&mut bindings, KeyCode::Char('n'), KeyModifiers::SHIFT, ActionId::PrevSearchResult, "Prev", "Previous search result", BindingContext::Always);

        // Follow mode
        push1(&mut bindings, KeyCode::Char('f'), KeyModifiers::SHIFT, ActionId::ToggleFollow, "Follow", "Toggle follow/tail mode", BindingContext::Always);

        // Parameters prefix
        push2(&mut bindings, 'p', 'r', ActionId::ToggleRegex, "Regex", "Toggle regex on/off", BindingContext::FilterPane);
        push2(&mut bindings, 'p', 'n', ActionId::ToggleNegate, "Negate", "Toggle negate filter", BindingContext::FilterPane);
        push2(&mut bindings, 'p', 'b', ActionId::ToggleInterleave, "Bookmarks", "Toggle viewing bookmarked lines", BindingContext::FilterPane);

        // Visual mode bindings
        push1(&mut bindings, KeyCode::Char('y'), KeyModifiers::empty(), ActionId::Yank, "Yank", "Copy selected lines to clipboard", BindingContext::VisualMode);

        Self { bindings }
    }

    pub fn lookup(&self, sequence: &[KeyCombo]) -> LookupResult {
        if sequence.is_empty() { return LookupResult::NoMatch; }

        let mut partials = Vec::new();
        let mut exact = None;

        for b in &self.bindings {
            if b.sequence.starts_with(sequence) {
                if b.sequence.len() == sequence.len() {
                    exact = Some(b);
                } else {
                    partials.push(b);
                }
            }
        }

        if let Some(e) = exact {
            LookupResult::ExactMatch(e.action)
        } else if !partials.is_empty() {
            LookupResult::PartialMatch
        } else {
            LookupResult::NoMatch
        }
    }

    pub fn visible_bindings(&self, context: BindingContextWrapper, pending: &[KeyCombo]) -> Vec<&KeyBinding> {
        let mut seen = std::collections::HashSet::new();
        let mut visible = Vec::new();

        for b in &self.bindings {
            if !b.sequence.starts_with(pending) { continue; }
            
            let context_match = match (b.context, context) {
                (BindingContext::Always, BindingContextWrapper::VisualMode) => {
                    matches!(b.action, ActionId::ScrollDown | ActionId::ScrollUp | ActionId::Quit)
                },
                (BindingContext::Always, _) => true,
                (BindingContext::FilterPane, BindingContextWrapper::FilterPane) => true,
                (BindingContext::MainPane, BindingContextWrapper::MainPane) => true,
                (BindingContext::VisualMode, BindingContextWrapper::VisualMode) => true,
                _ => false,
            };
            
            if context_match && seen.insert(b.action) {
                visible.push(b);
            }
        }
        visible
    }

    pub fn all_bindings(&self) -> &[KeyBinding] {
        &self.bindings
    }
}

pub enum LookupResult {
    ExactMatch(ActionId),
    PartialMatch,
    NoMatch,
}

#[derive(Clone, Copy)]
pub enum BindingContextWrapper {
    MainPane,
    FilterPane,
    VisualMode,
}

pub enum Action {
    Quit,
    ScrollDown,
    ScrollUp,
    NextPane,
    PrevPane,
    FocusPane(usize),
    ClosePane,
    CloseOtherPanes,
    SubmitFilter(String, FilterIntent), 
    EditFilter,
    ToggleRegex,
    ToggleNegate,
    ToggleInterleave,
    ToggleBookmark,
    ShowHelp,
    Yank(usize),
    EnterVisual,
    // Navigation
    GotoTop,
    GotoBottom,
    HalfPageDown,
    HalfPageUp,
    PageDown,
    PageUp,
    // Horizontal
    ScrollLeft,
    ScrollRight,
    // Search
    BeginSearch,
    SubmitSearch(String),
    NextSearchResult,
    PrevSearchResult,
    ClearSearch,
    // Follow
    ToggleFollow,
    None,
}

pub struct CommandHandler {
    pub mode: Mode,
    pub filter_input: String,
    pub filter_intent: FilterIntent,
    pub help_selected: usize,
    pub pending_keys: Vec<KeyCombo>,
    pub pending_timeout: std::time::Instant,
    pub registry: KeyRegistry,
    // Search state
    pub search_input: String,
    pub search_query: Option<String>,
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
        }
    }

    pub fn check_timeout(&mut self) -> Action {
        if !self.pending_keys.is_empty() && self.pending_timeout.elapsed().as_millis() > 1500 {
            self.pending_keys.clear();
        }
        Action::None
    }

    pub fn handle_key(&mut self, key: KeyEvent, current_line: usize) -> Action {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
            return Action::Quit;
        }

        self.check_timeout(); // flush if late

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

    fn handle_help(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
                self.mode = Mode::Normal;
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let items_count = self.registry.all_bindings().len();
                if self.help_selected + 1 < items_count {
                    self.help_selected += 1;
                }
                Action::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.help_selected = self.help_selected.saturating_sub(1);
                Action::None
            }
            KeyCode::Enter => {
                self.mode = Mode::Normal;
                let action_id = self.registry.all_bindings()[self.help_selected].action;
                self.execute_action(action_id, 0)
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
            ActionId::StackFilter => {
                self.filter_intent = FilterIntent::Stack;
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
            // Navigation
            ActionId::GotoTop => Action::GotoTop,
            ActionId::GotoBottom => Action::GotoBottom,
            ActionId::HalfPageDown => Action::HalfPageDown,
            ActionId::HalfPageUp => Action::HalfPageUp,
            ActionId::PageDown => Action::PageDown,
            ActionId::PageUp => Action::PageUp,
            // Horizontal
            ActionId::ScrollLeft => Action::ScrollLeft,
            ActionId::ScrollRight => Action::ScrollRight,
            // Search
            ActionId::BeginSearch => {
                self.search_input.clear();
                self.mode = Mode::Search;
                Action::BeginSearch
            }
            ActionId::NextSearchResult => Action::NextSearchResult,
            ActionId::PrevSearchResult => Action::PrevSearchResult,
            // Follow
            ActionId::ToggleFollow => Action::ToggleFollow,
        }
    }
}
