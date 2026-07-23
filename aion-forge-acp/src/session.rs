use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use anyhow::{bail, Result};
use tokio::sync::RwLock;
use uuid::Uuid;

const MAX_HISTORY_ENTRIES: usize = 48;

/// A normalized conversation or tool entry retained for future planning turns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryEntry {
    /// Content supplied by the user.
    User(String),
    /// Visible content returned by the assistant.
    Assistant(String),
    /// A normalized observation returned by a Forge capability.
    Tool {
        /// Executed capability name.
        name: String,
        /// Normalized capability result or error.
        observation: String,
    },
}

/// How an incoming prompt was stored by the session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptDisposition {
    /// An explicit AionUI bootstrap envelope was stored as an instruction.
    BootstrapStored,
    /// Ordinary content was appended to conversation history as a user turn.
    UserTurn,
}

/// Immutable session data returned without exposing the underlying state lock.
#[derive(Debug, Clone)]
pub struct SessionSnapshot {
    /// ACP session identifier.
    pub id: String,
    /// Validated working directory for tool execution.
    pub cwd: PathBuf,
    /// Model selector persisted for this session.
    pub selected_model: String,
    /// Explicit bootstrap instructions injected by AionUI.
    pub instructions: Vec<String>,
    /// Bounded conversation and tool history.
    pub history: Vec<HistoryEntry>,
    /// Cancellation flag for the currently active prompt.
    pub cancellation: Arc<AtomicBool>,
}

#[derive(Debug)]
struct SessionState {
    id: String,
    cwd: PathBuf,
    selected_model: String,
    instructions: Vec<String>,
    history: Vec<HistoryEntry>,
    cancellation: Arc<AtomicBool>,
}

impl SessionState {
    fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            id: self.id.clone(),
            cwd: self.cwd.clone(),
            selected_model: self.selected_model.clone(),
            instructions: self.instructions.clone(),
            history: self.history.clone(),
            cancellation: Arc::clone(&self.cancellation),
        }
    }

    fn append_history(&mut self, entry: HistoryEntry) {
        self.history.push(entry);
        let excess = self.history.len().saturating_sub(MAX_HISTORY_ENTRIES);
        if excess > 0 {
            self.history.drain(..excess);
        }
    }
}

/// Concurrent in-memory state for ACP sessions owned by one Forge process.
#[derive(Debug, Clone, Default)]
pub struct SessionStore {
    sessions: Arc<RwLock<HashMap<String, SessionState>>>,
}

impl SessionStore {
    /// Create a session after validating that its working directory exists.
    pub async fn create(&self, cwd: PathBuf, selected_model: String) -> Result<String> {
        if !cwd.is_dir() {
            bail!(
                "session working directory is not an existing directory: {}",
                cwd.display()
            );
        }

        let session_id = format!("forge-{}", Uuid::new_v4());
        let state = SessionState {
            id: session_id.clone(),
            cwd,
            selected_model,
            instructions: Vec::new(),
            history: Vec::new(),
            cancellation: Arc::new(AtomicBool::new(false)),
        };
        self.sessions.write().await.insert(session_id.clone(), state);
        Ok(session_id)
    }

    /// Return a cloned snapshot for a known session ID.
    pub async fn snapshot(&self, session_id: &str) -> Result<SessionSnapshot> {
        self.sessions
            .read()
            .await
            .get(session_id)
            .map(SessionState::snapshot)
            .ok_or_else(|| anyhow::anyhow!("unknown ACP session '{session_id}'"))
    }

    /// Persist the model selector supplied by the ACP client.
    pub async fn set_model(&self, session_id: &str, selected_model: String) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("unknown ACP session '{session_id}'"))?;
        session.selected_model = selected_model;
        Ok(())
    }

    /// Store explicit bootstrap envelopes separately from ordinary user turns.
    pub async fn ingest_prompt(&self, session_id: &str, prompt: &str) -> Result<PromptDisposition> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("unknown ACP session '{session_id}'"))?;
        let trimmed = prompt.trim();

        if trimmed.starts_with("[Assistant Rules]") || trimmed.starts_with("[Skill: ") {
            session.instructions.push(trimmed.to_string());
            return Ok(PromptDisposition::BootstrapStored);
        }

        session.append_history(HistoryEntry::User(prompt.to_string()));
        Ok(PromptDisposition::UserTurn)
    }

    /// Append one normalized entry and enforce the bounded history limit.
    pub async fn append_history(&self, session_id: &str, entry: HistoryEntry) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("unknown ACP session '{session_id}'"))?;
        session.append_history(entry);
        Ok(())
    }

    /// Cancel any previous prompt and install a fresh flag for the next prompt run.
    pub async fn start_prompt(&self, session_id: &str) -> Result<Arc<AtomicBool>> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("unknown ACP session '{session_id}'"))?;
        session.cancellation.store(true, Ordering::SeqCst);
        let cancellation = Arc::new(AtomicBool::new(false));
        session.cancellation = Arc::clone(&cancellation);
        Ok(cancellation)
    }

    /// Mark the active prompt for a known session as cancelled.
    pub async fn cancel(&self, session_id: &str) -> Result<()> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| anyhow::anyhow!("unknown ACP session '{session_id}'"))?;
        session.cancellation.store(true, Ordering::SeqCst);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::{atomic::Ordering, Arc},
    };

    use super::{HistoryEntry, PromptDisposition, SessionStore};

    fn existing_directory() -> PathBuf {
        std::env::current_dir().unwrap()
    }

    async fn create_session(store: &SessionStore) -> String {
        store.create(existing_directory(), "auto".to_string()).await.unwrap()
    }

    #[tokio::test]
    async fn creates_session_and_persists_selected_model() {
        let store = SessionStore::default();
        let session_id = create_session(&store).await;

        store.set_model(&session_id, "second-model".to_string()).await.unwrap();
        let snapshot = store.snapshot(&session_id).await.unwrap();

        assert_eq!(snapshot.id, session_id);
        assert_eq!(snapshot.cwd, existing_directory());
        assert_eq!(snapshot.selected_model, "second-model");
    }

    #[tokio::test]
    async fn rejects_missing_working_directory_and_unknown_session() {
        let store = SessionStore::default();
        let missing = existing_directory().join("missing-session-directory-for-test");
        let regular_file = existing_directory().join("Cargo.toml");

        assert!(store.create(missing, "auto".to_string()).await.is_err());
        assert!(store.create(regular_file, "auto".to_string()).await.is_err());
        assert!(store.snapshot("missing-session").await.is_err());
        assert!(store.set_model("missing-session", "auto".to_string()).await.is_err());
        assert!(store.cancel("missing-session").await.is_err());
    }

    #[tokio::test]
    async fn bootstrap_prompt_becomes_instruction_not_user_history() {
        let store = SessionStore::default();
        let session_id = create_session(&store).await;

        let disposition = store
            .ingest_prompt(&session_id, "  [Skill: aion-forge]\nUse Forge tools first.")
            .await
            .unwrap();

        assert_eq!(disposition, PromptDisposition::BootstrapStored);
        let snapshot = store.snapshot(&session_id).await.unwrap();
        assert_eq!(snapshot.instructions.len(), 1);
        assert!(snapshot.history.is_empty());
    }

    #[tokio::test]
    async fn assistant_rules_prompt_is_also_bootstrap() {
        let store = SessionStore::default();
        let session_id = create_session(&store).await;

        let disposition = store
            .ingest_prompt(&session_id, "[Assistant Rules]\nAnswer visibly.")
            .await
            .unwrap();

        assert_eq!(disposition, PromptDisposition::BootstrapStored);
    }

    #[tokio::test]
    async fn ordinary_skill_question_remains_a_user_turn() {
        let store = SessionStore::default();
        let session_id = create_session(&store).await;

        let disposition = store.ingest_prompt(&session_id, "你有哪些技能？").await.unwrap();

        assert_eq!(disposition, PromptDisposition::UserTurn);
        assert_eq!(
            store.snapshot(&session_id).await.unwrap().history,
            vec![HistoryEntry::User("你有哪些技能？".to_string())]
        );
    }

    #[tokio::test]
    async fn keeps_only_the_latest_48_history_entries() {
        let store = SessionStore::default();
        let session_id = create_session(&store).await;

        for index in 0..60 {
            store
                .append_history(&session_id, HistoryEntry::Assistant(index.to_string()))
                .await
                .unwrap();
        }

        let history = store.snapshot(&session_id).await.unwrap().history;
        assert_eq!(history.len(), 48);
        assert_eq!(history.first(), Some(&HistoryEntry::Assistant("12".to_string())));
        assert_eq!(history.last(), Some(&HistoryEntry::Assistant("59".to_string())));
    }

    #[tokio::test]
    async fn starting_a_prompt_cancels_and_replaces_the_active_flag() {
        let store = SessionStore::default();
        let session_id = create_session(&store).await;

        let first = store.start_prompt(&session_id).await.unwrap();
        let second = store.start_prompt(&session_id).await.unwrap();

        assert!(first.load(Ordering::SeqCst));
        assert!(!second.load(Ordering::SeqCst));
        assert!(!Arc::ptr_eq(&first, &second));
        assert!(Arc::ptr_eq(
            &second,
            &store.snapshot(&session_id).await.unwrap().cancellation
        ));

        store.cancel(&session_id).await.unwrap();
        assert!(second.load(Ordering::SeqCst));
    }
}
