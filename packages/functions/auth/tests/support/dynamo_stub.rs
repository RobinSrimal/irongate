//! A real HTTP boundary for SDK serialization, not a DynamoDB emulator.

use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Router,
};
use serde_json::Value;
use std::{
    collections::VecDeque,
    io,
    sync::{Arc, Mutex},
};
use tokio::{net::TcpListener, task::JoinHandle};

pub struct Reply {
    pub status: StatusCode,
    pub body: String,
    pub headers: HeaderMap,
}

impl Reply {
    pub fn json(status: StatusCode, value: Value) -> Self {
        Self {
            status,
            body: value.to_string(),
            headers: HeaderMap::new(),
        }
    }
}

#[derive(Default)]
struct StateData {
    replies: VecDeque<Reply>,
    requests: Vec<(String, Value)>,
}

pub struct Stub {
    pub endpoint: String,
    state: Arc<Mutex<StateData>>,
    task: JoinHandle<()>,
}

impl Stub {
    pub async fn start(replies: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let state = Arc::new(Mutex::new(StateData {
            replies: replies.into(),
            requests: vec![],
        }));
        let router = Router::new()
            .route(
                "/",
                post(
                    |State(state): State<Arc<Mutex<StateData>>>,
                     headers: HeaderMap,
                     body: Bytes| async move {
                        let mut state = state.lock().unwrap();
                        state.requests.push((
                            headers["x-amz-target"].to_str().unwrap().to_string(),
                            serde_json::from_slice(&body).unwrap(),
                        ));
                        let reply = state.replies.pop_front().unwrap_or_else(|| {
                            Reply::json(StatusCode::INTERNAL_SERVER_ERROR, serde_json::json!({}))
                        });
                        (reply.status, reply.headers, reply.body)
                    },
                ),
            )
            .with_state(state.clone());
        let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        Self {
            endpoint,
            state,
            task,
        }
    }

    pub fn requests(&self) -> Vec<(String, Value)> {
        self.state.lock().unwrap().requests.clone()
    }
}

impl Drop for Stub {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Clone, Default)]
pub struct Logs(Arc<Mutex<Vec<u8>>>);

impl Logs {
    pub fn text(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }

    pub fn subscriber(&self) -> impl tracing::Subscriber + Send + Sync + 'static {
        let writer = self.clone();
        tracing_subscriber::fmt()
            .with_ansi(false)
            .without_time()
            .with_max_level(tracing::Level::WARN)
            .with_writer(move || writer.clone())
            .finish()
    }
}

impl io::Write for Logs {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
