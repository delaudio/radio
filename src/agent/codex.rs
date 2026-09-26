use std::{path::Path, process::Stdio};

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use tokio::{io::{AsyncBufReadExt, AsyncWriteExt, BufReader}, process::{Child, ChildStdin, Command}, sync::broadcast, task::JoinHandle};

use super::{Agent, AgentEvent, AgentSession, SessionStatus};

pub struct CodexSession {
    child: Child,
    stdin: Option<ChildStdin>,
    events: broadcast::Sender<AgentEvent>,
    stdout_task: JoinHandle<()>,
    stderr_task: JoinHandle<()>,
    status: SessionStatus,
}

impl CodexSession {
    pub async fn start(repo: &Path) -> Result<Self> {
        let mut child = Command::new("codex")
            .current_dir(repo)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .context("failed to start Codex; is `codex` installed and available on PATH?")?;

        let stdin = child.stdin.take().context("Codex stdin was not piped")?;
        let stdout = child.stdout.take().context("Codex stdout was not piped")?;
        let stderr = child.stderr.take().context("Codex stderr was not piped")?;
        let (events, _) = broadcast::channel(256);
        let stdout_task = forward_lines(stdout, events.clone());
        let stderr_task = forward_lines(stderr, events.clone());

        Ok(Self { child, stdin: Some(stdin), events, stdout_task, stderr_task, status: SessionStatus::Running })
    }
}

fn forward_lines<R>(reader: R, events: broadcast::Sender<AgentEvent>) -> JoinHandle<()>
where R: tokio::io::AsyncRead + Unpin + Send + 'static {
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = events.send(AgentEvent::Output(line));
        }
    })
}

#[async_trait]
impl AgentSession for CodexSession {
    fn agent(&self) -> Agent { Agent::Codex }
    fn status(&self) -> SessionStatus { self.status }
    fn subscribe(&self) -> broadcast::Receiver<AgentEvent> { self.events.subscribe() }

    async fn send(&mut self, prompt: &str) -> Result<()> {
        if self.status != SessionStatus::Running { bail!("cannot send prompt to a stopped Codex session"); }
        let stdin = self.stdin.as_mut().context("Codex stdin is closed")?;
        stdin.write_all(prompt.as_bytes()).await.context("failed to write prompt to Codex")?;
        stdin.write_all(b"\n").await.context("failed to terminate Codex prompt")?;
        stdin.flush().await.context("failed to flush Codex stdin")
    }

    async fn stop(&mut self) -> Result<()> {
        if self.status == SessionStatus::Stopped { return Ok(()); }
        self.stdin.take();
        if self.child.try_wait()?.is_none() { self.child.kill().await.context("failed to stop Codex process")?; }
        self.stdout_task.abort();
        self.stderr_task.abort();
        self.status = SessionStatus::Stopped;
        let _ = self.events.send(AgentEvent::Stopped);
        Ok(())
    }
}

impl Drop for CodexSession {
    fn drop(&mut self) { self.stdout_task.abort(); self.stderr_task.abort(); }
}
