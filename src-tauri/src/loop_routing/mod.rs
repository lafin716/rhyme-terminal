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
    routing_capabilities(client).await.unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn capabilities_do_not_lock_or_initialize_engine() {
        let state = Arc::new(DaemonState::new());
        let guard = state.routing.engine.lock().await;
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            state
                .routing
                .request(&state, serde_json::json!({"op":"capabilities"})),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(result["version"], 1);
        assert!(guard.is_none());
    }
}

async fn routing_capabilities(client: &DaemonClient) -> Result<bool, String> {
    // A failed probe must not disable routing for the lifetime of the GUI.
    let value = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        client.request_raw(Method::LoopRequest {
            request: serde_json::json!({"op":"capabilities"}),
        }),
    )
    .await
    .map_err(|_| "데몬의 루프 기능 확인 시간이 초과되었습니다. 다시 시도하세요. 계속 실패하면 기존 작업을 마친 뒤 트레이의 ‘서버 종료’를 선택하고 최신 앱을 다시 실행하세요. 서버 종료 시 모든 터미널 세션이 종료됩니다.".to_string())?
    .map_err(|e| format!("데몬의 루프 기능을 확인하지 못했습니다: {e}"))?;
    Ok(value["version"] == 1)
}
#[tauri::command]
pub async fn loop_request(
    client: tauri::State<'_, Arc<DaemonClient>>,
    request: Value,
) -> Result<Value, String> {
    if !routing_capabilities(&client).await? {
        return Err("현재 데몬은 루프 라우팅을 지원하지 않습니다. 기존 작업을 마친 뒤 트레이의 ‘서버 종료’를 선택하고 최신 앱을 다시 실행하세요. ‘종료 (데몬 유지)’로는 업데이트되지 않습니다. 서버 종료 시 모든 터미널 세션이 종료됩니다.".into());
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
        // Discovery must not depend on persisted state or the engine lock.
        if request["op"] == "capabilities" {
            return Ok(
                serde_json::json!({"version":1,"runtimeMonitor":true,"autoStart":true,"livePolicy":true}),
            );
        }
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
