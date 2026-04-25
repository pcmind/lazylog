#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Filter,
    Search,
    Help,
    Visual { anchor_line: usize },
    LineDetail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionId {
    Quit,
    ScrollDown,
    ScrollUp,
    NextPane,
    PrevPane,
    NewFilter,
    EditFilter,
    ToggleRegex,
    ToggleNegate,
    ToggleInterleave,
    ToggleBookmark,
    TogglePinFilter,
    ToggleCaseSensitive,
    ClosePane,
    CloseOtherPanes,
    ShowHelp,
    Yank,
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
    NextSearchResult,
    PrevSearchResult,
    // Follow
    ToggleFollow,
    // Line detail
    ShowLineDetail,
    ToggleBoolean,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum BindingContext {
    Always,
    FilterPane,
    MainPane,
    VisualMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterIntent {
    New,
    Edit,
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
    TogglePinFilter,
    ToggleCaseSensitive,
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
    ShowLineDetail,
    ToggleBoolean,
    None,
}

#[derive(Clone, Copy)]
pub enum BindingContextWrapper {
    MainPane,
    FilterPane,
    VisualMode,
}
