use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::state::action::{ActionId, BindingContext, BindingContextWrapper};

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
    pub show_in_bar: bool,
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

        let push1 = |b: &mut Vec<KeyBinding>, code, modifiers, action, label, description, context, show_in_bar| {
            b.push(KeyBinding {
                sequence: vec![KeyCombo::new(code, modifiers)],
                action, label, description, context, show_in_bar
            });
        };

        let push2 = |b: &mut Vec<KeyBinding>, c1, c2, action, label, description, context, show_in_bar| {
            b.push(KeyBinding {
                sequence: vec![KeyCombo::new(KeyCode::Char(c1), KeyModifiers::empty()), KeyCombo::new(KeyCode::Char(c2), KeyModifiers::empty())],
                action, label, description, context, show_in_bar
            });
        };

        // Core navigation (hidden from bar — discoverable via ? help)
        push1(&mut bindings, KeyCode::Char('q'), KeyModifiers::empty(), ActionId::Quit, "Quit", "Quit lazylog", BindingContext::Always, true);
        push1(&mut bindings, KeyCode::Char('j'), KeyModifiers::empty(), ActionId::ScrollDown, "Down", "Scroll down", BindingContext::Always, false);
        push1(&mut bindings, KeyCode::Down, KeyModifiers::empty(), ActionId::ScrollDown, "Down", "Scroll down", BindingContext::Always, false);
        push1(&mut bindings, KeyCode::Char('k'), KeyModifiers::empty(), ActionId::ScrollUp, "Up", "Scroll up", BindingContext::Always, false);
        push1(&mut bindings, KeyCode::Up, KeyModifiers::empty(), ActionId::ScrollUp, "Up", "Scroll up", BindingContext::Always, false);

        // Page / half-page navigation (hidden)
        push1(&mut bindings, KeyCode::Char('d'), KeyModifiers::CONTROL, ActionId::HalfPageDown, "½PgDn", "Half page down", BindingContext::Always, false);
        push1(&mut bindings, KeyCode::Char('u'), KeyModifiers::CONTROL, ActionId::HalfPageUp, "½PgUp", "Half page up", BindingContext::Always, false);
        push1(&mut bindings, KeyCode::PageDown, KeyModifiers::empty(), ActionId::PageDown, "PgDn", "Page down", BindingContext::Always, false);
        push1(&mut bindings, KeyCode::PageUp, KeyModifiers::empty(), ActionId::PageUp, "PgUp", "Page up", BindingContext::Always, false);

        // Goto top/bottom (hidden)
        push1(&mut bindings, KeyCode::Char('g'), KeyModifiers::SHIFT, ActionId::GotoBottom, "Bottom", "Go to last line", BindingContext::Always, false);
        push2(&mut bindings, 'g', 'g', ActionId::GotoTop, "Top", "Go to first line", BindingContext::Always, false);

        // Horizontal scroll (hidden)
        push1(&mut bindings, KeyCode::Char('h'), KeyModifiers::empty(), ActionId::ScrollLeft, "Left", "Scroll left", BindingContext::Always, false);
        push1(&mut bindings, KeyCode::Left, KeyModifiers::empty(), ActionId::ScrollLeft, "Left", "Scroll left", BindingContext::Always, false);
        push1(&mut bindings, KeyCode::Char('l'), KeyModifiers::empty(), ActionId::ScrollRight, "Right", "Scroll right", BindingContext::Always, false);
        push1(&mut bindings, KeyCode::Right, KeyModifiers::empty(), ActionId::ScrollRight, "Right", "Scroll right", BindingContext::Always, false);

        // Pane navigation (hidden)
        push1(&mut bindings, KeyCode::Tab, KeyModifiers::empty(), ActionId::NextPane, "NextPane", "Focus next pane", BindingContext::Always, false);
        push1(&mut bindings, KeyCode::BackTab, KeyModifiers::SHIFT, ActionId::PrevPane, "PrevPane", "Focus previous pane", BindingContext::Always, false);

        // Filter (visible)
        push1(&mut bindings, KeyCode::Char('f'), KeyModifiers::empty(), ActionId::NewFilter, "Filter", "Create a new filter", BindingContext::Always, true);
        push1(&mut bindings, KeyCode::Char('f'), KeyModifiers::CONTROL, ActionId::NewFilter, "Filter", "Create a new filter", BindingContext::Always, false);
        push1(&mut bindings, KeyCode::Char('m'), KeyModifiers::empty(), ActionId::ToggleBookmark, "Mark", "Mark/Unmark selected line", BindingContext::Always, true);
        push1(&mut bindings, KeyCode::Char('p'), KeyModifiers::empty(), ActionId::TogglePinFilter, "Pin", "Toggle keeping last filter visible", BindingContext::Always, true);
        push1(&mut bindings, KeyCode::Char('x'), KeyModifiers::empty(), ActionId::ClosePane, "Close", "Close current pane", BindingContext::FilterPane, true);
        push1(&mut bindings, KeyCode::Char('x'), KeyModifiers::SHIFT, ActionId::CloseOtherPanes, "Close Other", "Close all other panes", BindingContext::Always, false);
        push1(&mut bindings, KeyCode::Char('?'), KeyModifiers::empty(), ActionId::ShowHelp, "Help", "Show keybindings menu", BindingContext::Always, true);

        // Search (visible)
        push1(&mut bindings, KeyCode::Char('/'), KeyModifiers::empty(), ActionId::BeginSearch, "Search", "Search in current view", BindingContext::MainPane, true);
        push1(&mut bindings, KeyCode::Char('n'), KeyModifiers::empty(), ActionId::NextSearchResult, "Next", "Next search result", BindingContext::MainPane, true);
        push1(&mut bindings, KeyCode::Char('n'), KeyModifiers::SHIFT, ActionId::PrevSearchResult, "Prev", "Previous search result", BindingContext::MainPane, true);

        // Follow mode (visible)
        push1(&mut bindings, KeyCode::Char('f'), KeyModifiers::SHIFT, ActionId::ToggleFollow, "Follow", "Toggle follow/tail mode", BindingContext::MainPane, true);

        // Edit Filter prefix (visible)
        push2(&mut bindings, 'e', 'e', ActionId::EditFilter, "Query", "Edit the filter query", BindingContext::FilterPane, true);
        push2(&mut bindings, 'e', 'r', ActionId::ToggleRegex, "Regex", "Toggle regex on/off", BindingContext::FilterPane, true);
        push2(&mut bindings, 'e', 'n', ActionId::ToggleNegate, "Negate", "Toggle negate filter", BindingContext::FilterPane, true);
        push2(&mut bindings, 'e', 'c', ActionId::ToggleCaseSensitive, "Case Sensitive", "Toggle case sensitive filter", BindingContext::FilterPane, true);
        push2(&mut bindings, 'e', 'b', ActionId::ToggleInterleave, "Bookmarks", "Toggle viewing bookmarked lines", BindingContext::FilterPane, true);

        // Visual mode bindings (visible)
        push1(&mut bindings, KeyCode::Char('y'), KeyModifiers::empty(), ActionId::Yank, "Yank", "Copy selected lines to clipboard", BindingContext::VisualMode, true);

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

    pub fn visible_bindings(&self, context: BindingContextWrapper, pending: &[KeyCombo], search_active: bool) -> Vec<&KeyBinding> {
        let mut seen = std::collections::HashSet::new();
        let mut visible = Vec::new();

        for b in &self.bindings {
            if !b.sequence.starts_with(pending) { continue; }

            // When no pending sequence, only show important bindings
            if pending.is_empty() && !b.show_in_bar { continue; }

            // Hide search navigation when no search is active
            if !search_active && matches!(b.action, ActionId::NextSearchResult | ActionId::PrevSearchResult) {
                continue;
            }

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
