use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Pane,
    Tab,
    Filter,
}

pub enum Action {
    Quit,
    NextMode(Mode),
    ScrollDown,
    ScrollUp,
    NextPane,
    PrevPane,
    FocusPane(usize),
    ClosePane,
    CloseOtherPanes,
    SubmitFilter(String, bool), 
    EditFilter,
    ToggleRegex,
    ToggleInterleave,
    ToggleBookmark,
    None,
}

pub struct Keybindings {
    pub quit: char,
    pub down: char,
    pub up: char,
    pub new_filter: char,
    pub edit_filter: char,
    pub stack_filter: char,
    pub toggle_regex: char,
    pub toggle_interleave: char,
    pub toggle_mark: char,
    pub close_pane: char,
    pub close_others: char,
}

impl Default for Keybindings {
    fn default() -> Self {
        Self {
            quit: 'q',
            down: 'j',
            up: 'k',
            new_filter: 'f',
            edit_filter: 'e',
            stack_filter: 's',
            toggle_regex: 'r',
            toggle_interleave: 'b',
            toggle_mark: 'm',
            close_pane: 'x',
            close_others: 'X',
        }
    }
}

pub struct CommandHandler {
    pub mode: Mode,
    pub filter_input: String,
    pub is_stacking: bool,
    pub keys: Keybindings,
}

impl CommandHandler {
    pub fn new() -> Self {
        Self { 
            mode: Mode::Normal,
            filter_input: String::new(),
            is_stacking: false,
            keys: Keybindings::default(),
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
            return Action::Quit;
        }

        match self.mode {
            Mode::Normal => self.handle_normal(key),
            Mode::Pane => self.handle_pane(key),
            Mode::Filter => self.handle_filter(key),
            Mode::Tab => self.handle_tab(key),
        }
    }

    fn check_char(&self, key: KeyCode, target: char) -> bool {
        if let KeyCode::Char(c) = key { c == target } else { false }
    }

    fn handle_normal(&mut self, key: KeyEvent) -> Action {
        let code = key.code;
        if self.check_char(code, self.keys.quit) { return Action::Quit; }
        if self.check_char(code, self.keys.down) || code == KeyCode::Down { return Action::ScrollDown; }
        if self.check_char(code, self.keys.up) || code == KeyCode::Up { return Action::ScrollUp; }
        if self.check_char(code, self.keys.toggle_mark) { return Action::ToggleBookmark; }
        if self.check_char(code, self.keys.toggle_regex) { return Action::ToggleRegex; }
        if self.check_char(code, self.keys.toggle_interleave) { return Action::ToggleInterleave; }
        if self.check_char(code, self.keys.close_pane) { return Action::ClosePane; }
        if self.check_char(code, self.keys.close_others) { return Action::CloseOtherPanes; }

        if self.check_char(code, self.keys.new_filter) {
            self.is_stacking = false;
            self.mode = Mode::Filter;
            return Action::NextMode(Mode::Filter);
        }

        if self.check_char(code, self.keys.edit_filter) {
            self.is_stacking = false;
            return Action::EditFilter;
        }
        if self.check_char(code, self.keys.stack_filter) {
            self.is_stacking = true;
            self.mode = Mode::Filter;
            return Action::NextMode(Mode::Filter);
        }

        // Direct digit focusing
        if let KeyCode::Char(c) = code {
            if c.is_ascii_digit() {
                return Action::FocusPane(c.to_digit(10).unwrap() as usize);
            }
        }

        // CTRL bindings
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match code {
                KeyCode::Char('p') => {
                    self.mode = Mode::Pane;
                    return Action::NextMode(Mode::Pane);
                }
                KeyCode::Char('t') => {
                    self.mode = Mode::Tab;
                    return Action::NextMode(Mode::Tab);
                }
                KeyCode::Char('f') => {
                    self.is_stacking = false;
                    self.mode = Mode::Filter;
                    return Action::NextMode(Mode::Filter);
                }
                _ => {}
            }
        }
        Action::None
    }

    fn handle_pane(&mut self, key: KeyEvent) -> Action {
        let code = key.code;
        if code == KeyCode::Esc {
            self.mode = Mode::Normal;
            return Action::NextMode(Mode::Normal);
        }
        if self.check_char(code, self.keys.down) || code == KeyCode::Down { return Action::NextPane; }
        if self.check_char(code, self.keys.up) || code == KeyCode::Up { return Action::PrevPane; }
        if self.check_char(code, 'n') { // kept separate alias for creation in pane mode
            self.is_stacking = false;
            self.mode = Mode::Filter;
            return Action::NextMode(Mode::Filter);
        }
        if self.check_char(code, self.keys.edit_filter) {
            self.is_stacking = false;
            return Action::EditFilter;
        }
        if self.check_char(code, self.keys.stack_filter) {
            self.is_stacking = true;
            self.mode = Mode::Filter;
            return Action::NextMode(Mode::Filter);
        }
        if self.check_char(code, self.keys.toggle_regex) { return Action::ToggleRegex; }
        if self.check_char(code, self.keys.toggle_interleave) { return Action::ToggleInterleave; }
        if self.check_char(code, self.keys.close_pane) { return Action::ClosePane; }
        if self.check_char(code, self.keys.close_others) { return Action::CloseOtherPanes; }

        if let KeyCode::Char(c) = code {
            if c.is_ascii_digit() {
                return Action::FocusPane(c.to_digit(10).unwrap() as usize);
            }
        }

        Action::None
    }

    fn handle_filter(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.filter_input.clear();
                self.is_stacking = false;
                Action::NextMode(Mode::Normal)
            }
            KeyCode::Enter => {
                self.mode = Mode::Pane; // Keep user context inside the pane after submit 
                let query = self.filter_input.clone();
                self.filter_input.clear();
                let stacking = self.is_stacking;
                self.is_stacking = false;
                if query.is_empty() {
                    Action::NextMode(Mode::Normal)
                } else {
                    Action::SubmitFilter(query, stacking)
                }
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

    fn handle_tab(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                Action::NextMode(Mode::Normal)
            }
            _ => Action::None,
        }
    }
}
