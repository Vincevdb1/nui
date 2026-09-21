use crate::components::command_log::LogEntry;
use crate::nix::{Input, Output};
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
    SetSuggestions(Result<Vec<(String, String)>, String>),
    SetPackageSearchResults(usize, Result<Vec<SearchResult>, String>),
    SetPackageDetails(
        usize,
        Result<HashMap<String, (String, String, bool, String)>, String>,
    ),
    AddPackageInfo(String, (String, String, bool, String)),
    SetContextData(Vec<Input>, Vec<Output>),
    SetVersions(Result<Vec<VersionInfo>, String>),
    UpdatePackageVersion(String, String),
    SetLockedVersion(usize, String, String, String), // search_id, attribute, channel, version

    // Context / State Refresh
    RefreshContext,
    FetchVersions(String),
    FetchShellPackageVersions(usize, String),

    // Mode switching
    SwitchMode,
    ToggleHelp,
    ToggleTemplates,
    ApplyTemplate(String),
    ConfirmApplyTemplate,
    CancelApplyTemplate,
    ToggleSaveShellTemplate,
    SaveShellTemplate,
    ApplyShellTemplate(Vec<String>),
    NewTemplateChar(char),
    NewTemplateBackspace,

    // Shell Mode Actions
    StartShell(Vec<String>),
    // Package selection
    TogglePackageSelection(String),
    ToggleShellPackageSelection(String),
    TogglePin(String),
    RemovePackage(usize),
    RemovePackages(Vec<usize>),
    RemoveFlakePackage(String),
    RemoveFlakePackages(Vec<String>),
    RemoveInput(String),

    // Version Selection
    SelectVersion(VersionInfo),
    SelectInputForPackage(String),
    MoveVersionSelectionDown,
    MoveVersionSelectionUp,
    MoveInputSelectionDown,
    MoveInputSelectionUp,
    MoveTemplateSelectionDown,
    MoveTemplateSelectionUp,

    // NXV Update
    UpdateNxvProgress(Option<String>),
}
