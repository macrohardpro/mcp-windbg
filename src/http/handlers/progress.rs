use axum::{
    extract::{Path, State},
    response::sse::{Event, KeepAlive, Sse},
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{debug, error, info};

use crate::http::{
    error::HttpError,
    session::SessionStatus,
};

/// Strip ANSI escape sequences from a string
fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // Skip escape sequence: ESC [ ... m or ESC ] ... BEL etc
            if let Some(next) = chars.next() {
                if next == '[' {
                    // CSI sequence: read until a letter (A-Z, a-z)
                    for c in chars.by_ref() {
                        if c.is_ascii_alphabetic() {
                            break;
                        }
                    }
                } else if next == ']' {
                    // OSC sequence: read until BEL or ST
                    for c in chars.by_ref() {
                        if c == '\u{07}' || c == '\u{1b}' {
                            break;
                        }
                    }
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

use super::AppState;

/// Progress event type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ProgressEvent {
    Status {
        status: String,
    },
    Progress {
        percent: f64,
    },
    Log {
        timestamp: String,
        message: String,
    },
    Complete {
        report_url: String,
    },
    Error {
        message: String,
    },
}

/// Handle progress streaming via Server-Sent Events
pub async fn handle_progress(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, HttpError> {
    info!("Progress stream requested for session {}", session_id);

    // Check if session exists
    let status = state
        .session_manager
        .get_session(&session_id)
        .await
        .ok_or_else(|| HttpError::SessionNotFound(session_id.clone()))?;

    debug!("Session {} status: {:?}", session_id, status);

    // Create event stream
    let stream = create_progress_stream(state, session_id);

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// Create progress event stream
fn create_progress_stream(
    state: AppState,
    session_id: String,
) -> impl Stream<Item = Result<Event, Infallible>> {
    async_stream::stream! {
        // Send initial status + progress percentage
        if let Some(status) = state.session_manager.get_session(&session_id).await {
            let status_str = format_status(&status);
            let pct = status_percent(&status);

            if let Ok(json) = serde_json::to_string(&ProgressEvent::Status { status: status_str }) {
                yield Ok(Event::default().data(json));
            }
            if let Ok(json) = serde_json::to_string(&ProgressEvent::Progress { percent: pct }) {
                yield Ok(Event::default().data(json));
            }
        }

        // Wait for process to be available
        let mut retries = 0;
        let process = loop {
            if let Some(proc) = state.session_manager.take_process(&session_id).await {
                break proc;
            }

            retries += 1;
            if retries > 30 {
                let ev = ProgressEvent::Error {
                    message: "分析进程未能启动".to_string(),
                };
                if let Ok(json) = serde_json::to_string(&ev) {
                    yield Ok(Event::default().data(json));
                }
                return;
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
        };

        info!("Streaming output for session {}", session_id);

        // Notify frontend analysis has started
        if let Ok(json) = serde_json::to_string(&ProgressEvent::Status { status: "分析中".to_string() }) {
            yield Ok(Event::default().data(json));
        }

        // Stream stdout and stderr
        let stdout = process.stdout;
        let stderr = process.stderr;

        if let (Some(stdout), Some(stderr)) = (stdout, stderr) {
            let stdout_reader = BufReader::new(stdout);
            let stderr_reader = BufReader::new(stderr);

            let mut stdout_lines = stdout_reader.lines();
            let mut stderr_lines = stderr_reader.lines();

            loop {
                tokio::select! {
                    result = stdout_lines.next_line() => {
                        match result {
                            Ok(Some(line)) => {
                                debug!("stdout: {}", line);

                                let ev = ProgressEvent::Log {
                                    timestamp: chrono::Utc::now().to_rfc3339(),
                                    message: strip_ansi(&line),
                                };
                                if let Ok(json) = serde_json::to_string(&ev) {
                                    yield Ok(Event::default().data(json));
                                }
                            }
                            Ok(None) => {
                                debug!("stdout closed");
                                break;
                            }
                            Err(e) => {
                                error!("Error reading stdout: {}", e);
                                break;
                            }
                        }
                    }
                    result = stderr_lines.next_line() => {
                        match result {
                            Ok(Some(line)) => {
                                debug!("stderr: {}", line);
                                let clean = strip_ansi(&line);

                                // Parse "--- Turn N/M ... ---" for progress %
                                if let Some(pct) = parse_turn_progress(&clean) {
                                    let ev = ProgressEvent::Progress { percent: pct };
                                    if let Ok(json) = serde_json::to_string(&ev) {
                                        yield Ok(Event::default().data(json));
                                    }
                                }

                                let ev = ProgressEvent::Log {
                                    timestamp: chrono::Utc::now().to_rfc3339(),
                                    message: clean,
                                };
                                if let Ok(json) = serde_json::to_string(&ev) {
                                    yield Ok(Event::default().data(json));
                                }
                            }
                            Ok(None) => {
                                debug!("stderr closed");
                                break;
                            }
                            Err(e) => {
                                error!("Error reading stderr: {}", e);
                                break;
                            }
                        }
                    }
                }
            }
        }

        // Check final status
        if let Some(status) = state.session_manager.get_session(&session_id).await {
            match status {
                SessionStatus::Complete => {
                    state.session_manager.update_status(&session_id, SessionStatus::Complete).await;

                    if let Ok(json) = serde_json::to_string(&ProgressEvent::Progress { percent: 100.0 }) {
                        yield Ok(Event::default().data(json));
                    }
                    let ev = ProgressEvent::Complete {
                        report_url: format!("/report/{}", session_id),
                    };
                    if let Ok(json) = serde_json::to_string(&ev) {
                        yield Ok(Event::default().data(json));
                    }
                }
                SessionStatus::Failed(msg) => {
                    let ev = ProgressEvent::Error { message: msg };
                    if let Ok(json) = serde_json::to_string(&ev) {
                        yield Ok(Event::default().data(json));
                    }
                }
                _ => {
                    state.session_manager.update_status(&session_id, SessionStatus::Complete).await;

                    if let Ok(json) = serde_json::to_string(&ProgressEvent::Progress { percent: 100.0 }) {
                        yield Ok(Event::default().data(json));
                    }
                    let ev = ProgressEvent::Complete {
                        report_url: format!("/report/{}", session_id),
                    };
                    if let Ok(json) = serde_json::to_string(&ev) {
                        yield Ok(Event::default().data(json));
                    }
                }
            }
        }

        info!("Progress stream ended for session {}", session_id);
    }
}

/// Format session status for display (Chinese labels)
fn format_status(status: &SessionStatus) -> String {
    match status {
        SessionStatus::Uploading => "上传中".to_string(),
        SessionStatus::Extracting => "解压中".to_string(),
        SessionStatus::Analyzing => "分析中".to_string(),
        SessionStatus::Complete => "完成".to_string(),
        SessionStatus::Failed(_) => "失败".to_string(),
    }
}

/// Get initial progress percentage for a session status
fn status_percent(status: &SessionStatus) -> f64 {
    match status {
        SessionStatus::Uploading => 5.0,
        SessionStatus::Extracting => 15.0,
        SessionStatus::Analyzing => 25.0,
        SessionStatus::Complete => 100.0,
        SessionStatus::Failed(_) => 0.0,
    }
}

/// Parse "--- Turn N/M ---" from Python MCP client stderr.
/// Returns progress percentage in 25-95% range.
fn parse_turn_progress(line: &str) -> Option<f64> {
    let line = line.trim();
    if !line.starts_with("--- Turn ") {
        return None;
    }
    let inner = line.strip_prefix("--- Turn ")?;
    let slash_pos = inner.find('/')?;
    let turn: f64 = inner[..slash_pos].parse().ok()?;
    let after = &inner[slash_pos + 1..];
    let max_str = after.split(|c: char| !c.is_ascii_digit()).next()?;
    let max_turns: f64 = max_str.parse().ok()?;
    if max_turns == 0.0 {
        return None;
    }
    let ratio = (turn / max_turns).clamp(0.0, 1.0);
    Some(((25.0 + ratio * 70.0) * 10.0).round() / 10.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_status() {
        assert_eq!(format_status(&SessionStatus::Uploading), "上传中");
        assert_eq!(format_status(&SessionStatus::Extracting), "解压中");
        assert_eq!(format_status(&SessionStatus::Analyzing), "分析中");
        assert_eq!(format_status(&SessionStatus::Complete), "完成");
        assert_eq!(format_status(&SessionStatus::Failed("error".to_string())), "失败");
    }

    #[test]
    fn test_status_percent() {
        assert_eq!(status_percent(&SessionStatus::Uploading), 5.0);
        assert_eq!(status_percent(&SessionStatus::Extracting), 15.0);
        assert_eq!(status_percent(&SessionStatus::Analyzing), 25.0);
        assert_eq!(status_percent(&SessionStatus::Complete), 100.0);
        assert_eq!(status_percent(&SessionStatus::Failed("err".to_string())), 0.0);
    }

    #[test]
    fn test_parse_turn_progress() {
        // Valid input
        let pct = parse_turn_progress("--- Turn 1/30 (elapsed 5s, ~1200 tokens) ---").unwrap();
        assert!(pct > 25.0 && pct < 30.0); // ~27.3%

        let pct = parse_turn_progress("--- Turn 15/30 ---").unwrap();
        assert!(pct > 55.0 && pct < 65.0); // ~60%

        let pct = parse_turn_progress("--- Turn 30/30 ---").unwrap();
        assert_eq!(pct, 95.0);

        // Invalid input
        assert!(parse_turn_progress("some random log").is_none());
        assert!(parse_turn_progress("Turn 5/30").is_none()); // no leading "--- "
        assert!(parse_turn_progress("--- Turn abc/30 ---").is_none());
        assert!(parse_turn_progress("--- Turn 1/0 ---").is_none()); // div by zero
    }

    #[test]
    fn test_progress_event_serialization() {
        let event = ProgressEvent::Status {
            status: "上传中".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"status\""));
        assert!(json.contains("\"status\":\"上传中\""));

        let event = ProgressEvent::Progress { percent: 45.0 };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"progress\""));
        assert!(json.contains("\"percent\":45.0"));

        let event = ProgressEvent::Log {
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            message: "test message".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"log\""));
        assert!(json.contains("\"message\":\"test message\""));

        let event = ProgressEvent::Complete {
            report_url: "/report/123".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"complete\""));
        assert!(json.contains("\"report_url\":\"/report/123\""));

        let event = ProgressEvent::Error {
            message: "error occurred".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"error\""));
        assert!(json.contains("\"message\":\"error occurred\""));
    }
}
