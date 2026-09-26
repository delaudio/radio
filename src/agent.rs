use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::broadcast;

pub mod codex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agent { Codex }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus { Running, Stopped }

#[derive(Debug, Clone)]
pub enum AgentEvent { Output(String), Stopped }

#[async_trait]
pub trait AgentSession: Send {
    fn agent(&self) -> Agent;
    fn status(&self) -> SessionStatus;
    fn subscribe(&self) -> broadcast::Receiver<AgentEvent>;
    async fn send(&mut self, prompt: &str) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
}

pub async fn start(agent: Agent, repo: &Path) -> Result<Box<dyn AgentSession>> {
    match agent {
        Agent::Codex => Ok(Box::new(codex::CodexSession::start(repo).await?)),
    }
}
