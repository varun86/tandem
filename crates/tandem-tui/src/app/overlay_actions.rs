use super::{App, AppState, ModalState, PagerOverlayState};
use std::path::PathBuf;

impl App {
    pub(super) fn open_file_search_modal(&mut self, initial_query: Option<&str>) {
        if let Some(query) = initial_query {
            self.file_search.query = query.to_string();
        }
        self.refresh_file_search_matches();
        if let AppState::Chat { modal, .. } = &mut self.state {
            *modal = Some(ModalState::FileSearch);
        }
    }

    pub(super) fn refresh_file_search_matches(&mut self) {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        self.file_search.matches =
            crate::ui::file_search::search_workspace_files(&cwd, &self.file_search.query, 80);
        if self.file_search.matches.is_empty() {
            self.file_search.cursor = 0;
        } else if self.file_search.cursor >= self.file_search.matches.len() {
            self.file_search.cursor = self.file_search.matches.len().saturating_sub(1);
        }
    }

    fn open_pager_overlay(&mut self, title: impl Into<String>, lines: Vec<String>, is_diff: bool) {
        self.pager_overlay = Some(PagerOverlayState {
            title: title.into(),
            lines,
            scroll: 0,
            is_diff,
        });
        if let AppState::Chat { modal, .. } = &mut self.state {
            *modal = Some(ModalState::Pager);
        }
    }

    pub(super) async fn open_diff_overlay(&mut self) -> String {
        match crate::ui::get_git_diff::get_git_diff().await {
            Ok((false, _)) => {
                "Cannot show diff: current directory is not a git repository.".to_string()
            }
            Ok((true, diff_text)) => {
                if diff_text.trim().is_empty() {
                    self.open_pager_overlay("Diff", vec!["No changes detected.".to_string()], true);
                } else {
                    self.open_pager_overlay(
                        "Diff",
                        diff_text.lines().map(|line| line.to_string()).collect(),
                        true,
                    );
                }
                "Opened structured diff overlay.".to_string()
            }
            Err(err) => format!("Failed to compute diff: {}", err),
        }
    }

    pub(super) async fn open_external_editor_for_active_input(&mut self) -> String {
        let seed = if let AppState::Chat { command_input, .. } = &self.state {
            command_input.text().to_string()
        } else {
            String::new()
        };
        match crate::ui::external_editor::run_editor(&seed).await {
            Ok(edited) => {
                if let AppState::Chat { command_input, .. } = &mut self.state {
                    command_input.set_text(edited);
                }
                self.sync_active_agent_from_chat();
                "Loaded edited draft from external editor.".to_string()
            }
            Err(err) => format!("External editor failed: {}", err),
        }
    }
}
