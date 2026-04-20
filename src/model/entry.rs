use crate::errors::IronaError;
use crate::scanner::ArtifactEntry;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct EntryModel {
    pub entry: ArtifactEntry,
    pub selected: bool,
    pub delete_state: DeleteState,
}

#[derive(Debug, Clone)]
pub enum DeleteState {
    Pending,
    Deleted { elapsed: Duration },
    Failed { message: String, elapsed: Duration },
}

#[derive(Debug)]
pub enum EntryMsg {
    DeleteResult {
        elapsed: Duration,
        outcome: Result<(), IronaError>,
    },
}

impl EntryModel {
    pub fn new(entry: ArtifactEntry) -> Self {
        Self {
            entry,
            selected: false,
            delete_state: DeleteState::Pending,
        }
    }

    pub fn apply(&mut self, msg: EntryMsg) {
        match msg {
            EntryMsg::DeleteResult {
                elapsed,
                outcome: Ok(()),
            } => {
                self.delete_state = DeleteState::Deleted { elapsed };
            }
            EntryMsg::DeleteResult {
                elapsed,
                outcome: Err(e),
            } => {
                self.delete_state = DeleteState::Failed {
                    message: e.to_string(),
                    elapsed,
                };
            }
        }
    }
}
