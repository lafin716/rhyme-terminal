pub mod adapter;
pub mod bridge;
pub mod model;
mod process_controller;
mod runtime;

use crate::ipc::{client::DaemonClient, protocol::Method, server::DaemonState};
use anyhow::Result;
use parking_lot::Mutex;
use serde_json::Value;
use std::{collections::HashSet, sync::Arc};
use tokio::sync::Mutex as AsyncMutex;

#[derive(Default)]
pub struct Service {
    engine: AsyncMutex<Option<runtime::Engine>>,
    pub owned: Mutex<HashSet<uuid::Uuid>>,
}

pub async fn daemon_supports_routing(client: &DaemonClient) -> bool {
    static SUPPORTED: tokio::sync::OnceCell<bool> = tokio::sync::OnceCell::const_new();
    *SUPPORTED.get_or_init(|| async {
        matches!(tokio::time::timeout(std::time::Duration::from_secs(3),
            client.request_raw(Method::LoopRequest {request: serde_json::json!({"op":"capabilities"})})).await,
            Ok(Ok(value)) if value["version"] == 1)
    }).await
}
#[tauri::command]
pub async fn loop_request(
    client: tauri::State<'_, Arc<DaemonClient>>,
    request: Value,
) -> Result<Value, String> {
    if !daemon_supports_routing(&client).await {
        return Err("현재 데몬은 루프 라우팅을 지원하지 않습니다. 기존 작업을 마친 뒤 트레이에서 종료하고 새 버전 앱을 실행하세요. 일반 터미널은 계속 사용할 수 있습니다.".into());
    }
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client.request_raw(Method::LoopRequest { request }),
    )
    .await
    .map_err(|_| {
        "루프 요청 응답 시간이 초과되었습니다. 그룹 목록에서 결과를 확인하세요.".to_string()
    })?
    .map_err(|e| format!("루프 라우팅 요청 실패: {e}"))
}
impl Service {
    pub async fn request(&self, state: &Arc<DaemonState>, request: Value) -> Result<Value> {
        let mut guard = self.engine.lock().await;
        if guard.is_none() {
            *guard = Some(runtime::Engine::open()?);
        }
        let engine = guard.as_mut().unwrap();
        let result = engine.request(state, request);
        engine.sync_guards(self);
        result
    }
    pub async fn write_managed(
        &self,
        state: &Arc<DaemonState>,
        id: uuid::Uuid,
        bytes: &[u8],
    ) -> Result<bool> {
        if !self.owned.lock().contains(&id) {
            return Ok(false);
        }
        let mut engine = self.engine.lock().await;
        match engine.as_mut() {
            Some(engine) => engine.write_managed(state, id, bytes),
            None => Ok(false),
        }
    }
    pub fn start(state: Arc<DaemonState>) {
        let ticker = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            loop {
                interval.tick().await;
                let mut engine = ticker.routing.engine.lock().await;
                if let Some(engine) = engine.as_mut() {
                    if let Err(error) = engine.tick(&ticker) {
                        tracing::warn!("loop routing tick: {error}");
                    }
                    engine.sync_guards(&ticker.routing);
                }
            }
        });
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            loop {
                interval.tick().await;
                let targets = {
                    let mut engine = state.routing.engine.lock().await;
                    engine
                        .as_mut()
                        .map(|e| e.usage_targets())
                        .unwrap_or_default()
                };
                let mut pending = tokio::task::JoinSet::new();
                let mut targets = targets.into_iter();
                loop {
                    while pending.len() < 2 {
                        let Some((key, agent, dir, session_usage, requested_at)) = targets.next()
                        else {
                            break;
                        };
                        pending.spawn(async move {
                            let result = tokio::time::timeout(
                                std::time::Duration::from_secs(20),
                                crate::usage::refresh_account_usage(&agent, &dir, session_usage),
                            )
                            .await
                            .unwrap_or_else(|_| Err("Usage refresh timeout".into()));
                            (key, result, requested_at)
                        });
                    }
                    let Some(result) = pending.join_next().await else {
                        break;
                    };
                    if let Ok((key, usage, requested_at)) = result {
                        if let Some(engine) = state.routing.engine.lock().await.as_mut() {
                            engine.quota(key.clone(), usage, requested_at);
                            // Threshold evidence acts immediately, not at the next tick.
                            if let Err(error) = engine.tick_for_profile(&state, &key) {
                                tracing::warn!("usage guard: {error}");
                            }
                            engine.sync_guards(&state.routing);
                        }
                    }
                }
            }
        });
    }
}
