use std::{collections::HashMap, path::Path, sync::Arc};

use anyhow::{Result, bail};
use tokio::sync::{Mutex, broadcast};
use uuid::Uuid;

use crate::agent::{self, Agent, AgentEvent, AgentSession, SessionStatus};

pub type SessionId = Uuid;

pub struct Session { pub id: SessionId, inner: Box<dyn AgentSession> }

impl Session {
    pub fn agent(&self) -> Agent { self.inner.agent() }
    pub fn status(&self) -> SessionStatus { self.inner.status() }
    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> { self.inner.subscribe() }
    pub async fn send(&mut self, prompt: &str) -> Result<()> { self.inner.send(prompt).await }
    pub async fn stop(&mut self) -> Result<()> { self.inner.stop().await }
}

#[derive(Clone, Default)]
pub struct SessionManager { sessions: Arc<Mutex<HashMap<SessionId, Arc<Mutex<Session>>>>> }

impl SessionManager {
    pub async fn create(&self, agent_kind: Agent, repo: &Path) -> Result<SessionId> {
        let inner = agent::start(agent_kind, repo).await?;
        let id = Uuid::new_v4();
        self.sessions.lock().await.insert(id, Arc::new(Mutex::new(Session { id, inner })));
        Ok(id)
    }

    pub async fn get(&self, id: SessionId) -> Option<Arc<Mutex<Session>>> { self.sessions.lock().await.get(&id).cloned() }

    pub async fn stop(&self, id: SessionId) -> Result<()> {
        let Some(session) = self.get(id).await else { bail!("unknown session: {id}"); };
        session.lock().await.stop().await
    }

    pub async fn stop_all(&self) {
        let sessions: Vec<_> = self.sessions.lock().await.values().cloned().collect();
        for session in sessions { let _ = session.lock().await.stop().await; }
    }
}
