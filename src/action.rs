use crate::components::command_log::LogEntry;
use crate::state::domain::{SearchResult, VersionInfo};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Action {
    Tick,
    Quit,
    NextTab,
    PreviousTab,
    SelectTab(usize),

    // Selection navigation (Main screen)
    MoveDown,
    MoveUp,
    MovePackageSelectionDown,
    MovePackageSelectionUp,

    // Popups
    OpenAddPackage,
    OpenAddInput,
    ClosePopup,

    // Package Search Popup
    PackageSearchChar(char),
    PackageSearchBackspace,
    PackageSearchSubmitDirect,
    PackageSearchSubmitVersions,
    TogglePackageDetails,
    MoveSearchSelectionDown,
    MoveSearchSelectionUp,
    BackToPackageSearch,

    // Input Popup
    InputPopupChar(char),
    InputPopupBackspace,
    InputPopupSubmit,
    NextInputField,
    PreviousInputField,
    MoveSuggestionDown,
    MoveSuggestionUp,

    // Background Task Results
    Log(LogEntry),
    SetSuggestions(Vec<(String, String)>),
    SetPackageSearchResults(Result<Vec<SearchResult>, String>),
    SetPackageDetails(Result<HashMap<String, (String, String, bool, String)>, String>),
    SetVersions(Result<Vec<VersionInfo>, String>),
    UpdatePackageVersion(String, String),
    SetLockedVersion(String, String, String), // attribute, channel, version

    // Context / State Refresh
    RefreshContext,
    FetchPackageDetails,
    FetchVersions(String),

    // Mode switching
    SwitchMode,

    // Shell Mode Actions
    StartShell(Vec<String>),
    UpdateShellPackages(Vec<String>),
    RemovePackage(usize),
    RemoveFlakePackage(String),

    // Version Selection
    SelectVersion(VersionInfo),
    MoveVersionSelectionDown,
    MoveVersionSelectionUp,
}
