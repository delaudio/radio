mod agent;
mod api;
mod protocol;
mod session;

use std::{net::IpAddr, path::PathBuf};
use anyhow::{Context, Result, bail};
use axum::{Router, routing::{get, post}};
use clap::{Parser, Subcommand};
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::EnvFilter;
use api::AppState;
use session::SessionManager;

#[derive(Debug, Parser)]
#[command(name = "radio", version, about)]
struct Cli { #[command(subcommand)] command: Command }

#[derive(Debug, Subcommand)]
enum Command {
    /// Start the radio daemon for a repository.
    Serve {
        /// Repository the coding agent will operate in.
        repo: PathBuf,
        /// Address to listen on.
        #[arg(long, env = "RADIO_HOST", default_value = "127.0.0.1")]
        host: IpAddr,
        /// Port to listen on.
        #[arg(long, env = "RADIO_PORT", default_value_t = 8787)]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    match Cli::parse().command { Command::Serve { repo, host, port } => serve(repo, host, port).await }
}

async fn serve(repo: PathBuf, host: IpAddr, port: u16) -> Result<()> {
    let repo = validate_repo(repo)?;
    let listener = TcpListener::bind((host, port)).await.with_context(|| format!("failed to bind to {host}:{port}"))?;
    let sessions = SessionManager::default();
    let state = AppState { repo: repo.clone(), sessions: sessions.clone() };
    let app = Router::new()
        .route("/health", get(health))
        .route("/sessions", post(api::create_session))
        .route("/sessions/{id}/ws", get(api::session_socket))
        .with_state(state);
    info!(repository = %repo.display(), %host, %port, "radio daemon listening");
    axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()).await.context("radio server failed")?;
    sessions.stop_all().await;
    Ok(())
}

fn validate_repo(repo: PathBuf) -> Result<PathBuf> {
    if !repo.exists() { bail!("repository does not exist: {}", repo.display()); }
    if !repo.is_dir() { bail!("repository is not a directory: {}", repo.display()); }
    repo.canonicalize().with_context(|| format!("failed to resolve repository: {}", repo.display()))
}

async fn health() -> &'static str { "ok" }

async fn shutdown_signal() {
    let ctrl_c = async { tokio::signal::ctrl_c().await.expect("failed to install Ctrl-C handler"); };
    #[cfg(unix)]
    let terminate = async { tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("failed to install SIGTERM handler").recv().await; };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {} _ = terminate => {} }
    info!("shutdown signal received");
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("radio=info"));
    tracing_subscriber::fmt().with_env_filter(filter).compact().init();
}

#[cfg(test)]
mod tests {
    use super::validate_repo;
    #[test]
    fn rejects_missing_repository() {
        let missing = std::env::temp_dir().join(format!("radio-missing-repo-{}", std::process::id()));
        assert!(validate_repo(missing).is_err());
    }
    #[test]
    fn accepts_existing_directory() {
        let repo = std::env::current_dir().expect("current directory should exist");
        assert!(validate_repo(repo).is_ok());
    }
}
