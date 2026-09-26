use std::{path::PathBuf, sync::Arc};

use axum::{
    Json,
    extract::{Path, State, WebSocketUpgrade, ws::{Message, WebSocket}},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use tokio::{process::Command, sync::Mutex};

use crate::{
    agent::{Agent, AgentEvent, SessionStatus},
    protocol::{ClientMessage, ServerMessage},
    session::{Session, SessionId, SessionManager},
};

#[derive(Clone)]
pub struct AppState {
    pub repo: PathBuf,
    pub sessions: SessionManager,
}

#[derive(Serialize)]
pub struct DiffResponse { pub diff: String }

#[derive(Serialize)]
pub struct CreateSessionResponse {
    pub id: SessionId,
}

pub async fn create_session(
    State(state): State<AppState>,
) -> Result<Json<CreateSessionResponse>, ApiError> {
    let id = state.sessions.create(Agent::Codex, &state.repo).await.map_err(ApiError::internal)?;
    Ok(Json(CreateSessionResponse { id }))
}

pub async fn git_diff(State(state): State<AppState>) -> Result<Json<DiffResponse>, ApiError> {
    let output = Command::new("git")
        .args(["diff", "--no-ext-diff", "--"])
        .current_dir(&state.repo)
        .output()
        .await
        .map_err(ApiError::internal)?;
    if !output.status.success() {
        return Err(ApiError::internal(String::from_utf8_lossy(&output.stderr)));
    }
    Ok(Json(DiffResponse { diff: String::from_utf8_lossy(&output.stdout).into_owned() }))
}

pub async fn session_socket(
    Path(id): Path<SessionId>,
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let session = state.sessions.get(id).await
        .ok_or_else(|| ApiError::not_found(format!("unknown session: {id}")))?;
    Ok(ws.on_upgrade(move |socket| handle_socket(socket, session)))
}

async fn handle_socket(socket: WebSocket, session: Arc<Mutex<Session>>) {
    let (mut sender, mut receiver) = socket.split();
    let (mut events, initial_status) = {
        let session = session.lock().await;
        (session.subscribe(), session.status())
    };

    if send_json(&mut sender, &ServerMessage::Status {
        status: status_name(initial_status).to_owned(),
    }).await.is_err() {
        return;
    }

    loop {
        tokio::select! {
            incoming = receiver.next() => {
                let Some(Ok(message)) = incoming else { break; };
                match message {
                    Message::Text(text) => match serde_json::from_str::<ClientMessage>(&text) {
                        Ok(ClientMessage::Prompt { text }) => {
                            if let Err(error) = session.lock().await.send(&text).await {
                                if send_json(&mut sender, &ServerMessage::Error { message: error.to_string() }).await.is_err() { break; }
                            }
                        }
                        Ok(ClientMessage::Stop) => {
                            if let Err(error) = session.lock().await.stop().await {
                                if send_json(&mut sender, &ServerMessage::Error { message: error.to_string() }).await.is_err() { break; }
                            }
                        }
                        Err(error) => {
                            if send_json(&mut sender, &ServerMessage::Error { message: format!("invalid message: {error}") }).await.is_err() { break; }
                        }
                    },
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            event = events.recv() => match event {
                Ok(AgentEvent::Output(text)) => {
                    if send_json(&mut sender, &ServerMessage::Output { text }).await.is_err() { break; }
                }
                Ok(AgentEvent::Stopped) => {
                    let _ = send_json(&mut sender, &ServerMessage::Status { status: "stopped".to_owned() }).await;
                    let _ = send_json(&mut sender, &ServerMessage::Done).await;
                    break;
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    if send_json(&mut sender, &ServerMessage::Error {
                        message: format!("client lagged; skipped {skipped} output events"),
                    }).await.is_err() { break; }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    }
    // A disconnected mobile client must not stop the underlying agent session.
}

async fn send_json<S>(sender: &mut S, message: &ServerMessage) -> Result<(), ()>
where
    S: futures_util::Sink<Message> + Unpin,
{
    let json = serde_json::to_string(message).map_err(|_| ())?;
    sender.send(Message::Text(json.into())).await.map_err(|_| ())
}

fn status_name(status: SessionStatus) -> &'static str {
    match status {
        SessionStatus::Running => "running",
        SessionStatus::Stopped => "stopped",
    }
}

pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn internal(error: impl std::fmt::Display) -> Self {
        Self { status: StatusCode::INTERNAL_SERVER_ERROR, message: error.to_string() }
    }
    fn not_found(message: String) -> Self {
        Self { status: StatusCode::NOT_FOUND, message }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(ServerMessage::Error { message: self.message })).into_response()
    }
}
