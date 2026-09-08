use parking_lot::Mutex;
use serde::Serialize;
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::Arc,
};
use tokio::sync::watch;
use uuid::Uuid;

mod gateway;
const INVITE_MS: u64 = 120_000;
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn secret() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}
fn base_url(addr: SocketAddr) -> String {
    format!("http://{addr}/")
}
fn valid_headers(addr: SocketAddr, host: Option<&str>, origin: Option<&str>, ws: bool) -> bool {
    let authority = if addr.port() == 80 {
        match addr.ip() {
            IpAddr::V4(ip) => ip.to_string(),
            IpAddr::V6(ip) => format!("[{ip}]"),
        }
    } else {
        addr.to_string()
    };
    (host == Some(authority.as_str()) || host == Some(addr.to_string().as_str()))
        && match origin {
            Some(value) => value == format!("http://{authority}"),
            None => !ws,
        }
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkInterface {
    name: String,
    ip: String,
    is_tailscale: bool,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingDevice {
    request_id: String,
    name: String,
    verification: String,
}
struct Pending {
    view: PendingDevice,
    expires: u64,
    result: watch::Sender<Option<Result<String, String>>>,
}
struct Device {
    device_id: String,
    name: String,
    cancel: watch::Sender<bool>,
    connections: usize,
}
#[derive(Default)]
struct Pairings {
    invite: Option<(String, u64)>,
    pending: HashMap<String, Pending>,
    devices: HashMap<String, Device>,
}
impl Pairings {
    fn invite(&mut self, now: u64) -> String {
        let token = secret();
        self.invite = Some((token.clone(), now + INVITE_MS));
        token
    }
    fn expire(&mut self, now: u64) {
        if self
            .invite
            .as_ref()
            .is_some_and(|(_, expiry)| now >= *expiry)
        {
            self.invite = None;
        }
        self.pending.retain(|_, p| {
            if now >= p.expires {
                p.result
                    .send_replace(Some(Err("승인 시간이 만료되었습니다.".into())));
                false
            } else {
                true
            }
        });
    }
    fn pair(&mut self, token: &str, name: &str, now: u64) -> Result<PendingDevice, String> {
        self.expire(now);
        if self.pending.len() >= 8 || self.devices.len() >= 16 {
            return Err("등록 가능한 기기 수를 초과했습니다.".into());
        }
        if !self
            .invite
            .as_ref()
            .is_some_and(|(value, _)| value == token)
        {
            return Err("초대가 유효하지 않거나 만료되었습니다.".into());
        }
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 64 || name.chars().any(char::is_control) {
            return Err("기기 이름은 1~64자로 입력하세요.".into());
        }
        let (_, expires) = self.invite.take().unwrap();
        let view = PendingDevice {
            request_id: Uuid::new_v4().to_string(),
            name: name.into(),
            verification: format!("{:06}", Uuid::new_v4().as_u128() % 1_000_000),
        };
        self.pending.insert(
            view.request_id.clone(),
            Pending {
                view: view.clone(),
                expires,
                result: watch::channel(None).0,
            },
        );
        Ok(view)
    }
    fn approve(&mut self, id: &str, approve: bool, now: u64) -> Result<Option<String>, String> {
        self.expire(now);
        let p = self
            .pending
            .remove(id)
            .ok_or("승인 요청이 없거나 만료되었습니다.")?;
        if !approve {
            p.result
                .send_replace(Some(Err("PC에서 연결을 거절했습니다.".into())));
            return Ok(None);
        }
        let token = secret();
        self.devices.insert(
            token.clone(),
            Device {
                device_id: Uuid::new_v4().to_string(),
                name: p.view.name,
                cancel: watch::channel(false).0,
                connections: 0,
            },
        );
        p.result.send_replace(Some(Ok(token.clone())));
        Ok(Some(token))
    }
    #[cfg(test)]
    fn authenticate(&self, token: &str) -> Option<&Device> {
        self.devices.get(token)
    }
    fn revoke(&mut self, id: &str) -> Result<(), String> {
        let token = self
            .devices
            .iter()
            .find(|(_, d)| d.device_id == id)
            .map(|(t, _)| t.clone())
            .ok_or("등록된 기기가 없습니다.")?;
        let device = self.devices.remove(&token).unwrap();
        device.cancel.send_replace(true);
        Ok(())
    }
    fn clear(&mut self) {
        for d in self.devices.values() {
            d.cancel.send_replace(true);
        }
        for p in self.pending.values() {
            p.result
                .send_replace(Some(Err("모바일 연결 서버가 중지되었습니다.".into())));
        }
        *self = Self::default();
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InviteView {
    url: String,
    qr_svg: String,
    expires_at: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairedDevice {
    device_id: String,
    name: String,
    connected: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingStatus {
    running: bool,
    ip: Option<String>,
    port: Option<u16>,
    url: Option<String>,
    invite: Option<InviteView>,
    pending: Vec<PendingDevice>,
    devices: Vec<PairedDevice>,
}
struct Running {
    address: SocketAddr,
    stop: watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
}
#[derive(Default)]
pub struct MobilePairing {
    running: tokio::sync::Mutex<Option<Running>>,
    pairings: Arc<Mutex<Pairings>>,
}
impl Drop for MobilePairing {
    fn drop(&mut self) {
        if let Some(running) = self.running.get_mut().take() {
            running.stop.send_replace(true);
            running.task.abort();
        }
        self.pairings.lock().clear();
    }
}
impl MobilePairing {
    async fn status(&self) -> PairingStatus {
        let running = self.running.lock().await;
        let address = running
            .as_ref()
            .filter(|r| !r.task.is_finished())
            .map(|r| r.address);
        let mut p = self.pairings.lock();
        p.expire(now_ms());
        let invite = address.and_then(|addr| {
            p.invite.as_ref().map(|(token, expires)| {
                let url = format!("{}#invite={token}", base_url(addr));
                let code = qrcode::QrCode::new(url.as_bytes()).expect("bounded invite fits QR");
                InviteView {
                    url,
                    qr_svg: code
                        .render::<qrcode::render::svg::Color>()
                        .min_dimensions(240, 240)
                        .build(),
                    expires_at: *expires,
                }
            })
        });
        PairingStatus {
            running: address.is_some(),
            ip: address.map(|a| a.ip().to_string()),
            port: address.map(|a| a.port()),
            url: address.map(base_url),
            invite,
            pending: p.pending.values().map(|p| p.view.clone()).collect(),
            devices: p
                .devices
                .values()
                .map(|d| PairedDevice {
                    device_id: d.device_id.clone(),
                    name: d.name.clone(),
                    connected: d.connections > 0,
                })
                .collect(),
        }
    }
    async fn start(&self, ip: String, port: u16) -> Result<PairingStatus, String> {
        let mut current = self.running.lock().await;
        if current.as_ref().is_some_and(|r| !r.task.is_finished()) {
            return Err("먼저 모바일 연결 서버를 중지하세요.".into());
        }
        let ip: IpAddr = ip.parse().map_err(|_| "IP 주소가 올바르지 않습니다.")?;
        if port == 0 || !interfaces()?.iter().any(|i| i.ip == ip.to_string()) {
            return Err("선택한 IP 또는 포트를 사용할 수 없습니다. 목록을 새로고침하세요.".into());
        }
        let listener = tokio::net::TcpListener::bind(SocketAddr::new(ip, port))
            .await
            .map_err(|e| format!("선택한 주소에서 서버를 시작할 수 없습니다: {e}"))?;
        let address = listener.local_addr().map_err(|e| e.to_string())?;
        let (stop, rx) = watch::channel(false);
        self.pairings.lock().clear();
        self.pairings.lock().invite(now_ms());
        let task = tokio::spawn(gateway::serve(listener, address, self.pairings.clone(), rx));
        *current = Some(Running {
            address,
            stop,
            task,
        });
        drop(current);
        Ok(self.status().await)
    }
    async fn stop(&self) -> PairingStatus {
        let mut current = self.running.lock().await;
        if let Some(mut running) = current.take() {
            running.stop.send_replace(true);
            self.pairings.lock().clear();
            if tokio::time::timeout(std::time::Duration::from_secs(3), &mut running.task)
                .await
                .is_err()
            {
                running.task.abort();
                let _ = running.task.await;
            }
        }
        drop(current);
        self.status().await
    }
}
fn interfaces() -> Result<Vec<NetworkInterface>, String> {
    let mut result = if_addrs::get_if_addrs()
        .map_err(|e| format!("네트워크 목록을 읽을 수 없습니다: {e}"))?
        .into_iter()
        .filter(|i| usable_ip(i.ip()))
        .map(|i| NetworkInterface {
            ip: i.ip().to_string(),
            is_tailscale: i.name.to_ascii_lowercase().contains("tailscale"),
            name: i.name,
        })
        .collect::<Vec<_>>();
    result.sort_by(|a, b| {
        b.is_tailscale
            .cmp(&a.is_tailscale)
            .then(a.name.cmp(&b.name))
            .then(a.ip.cmp(&b.ip))
    });
    result.dedup_by(|a, b| a.ip == b.ip && a.name == b.name);
    Ok(result)
}
fn usable_ip(ip: IpAddr) -> bool {
    !ip.is_unspecified()
        && !ip.is_multicast()
        && match ip {
            IpAddr::V4(a) => !a.is_broadcast(),
            IpAddr::V6(a) => !a.is_unicast_link_local(),
        }
}
#[tauri::command]
pub async fn mobile_pairing_interfaces() -> Result<Vec<NetworkInterface>, String> {
    interfaces()
}
#[tauri::command]
pub async fn mobile_pairing_status(
    state: tauri::State<'_, MobilePairing>,
) -> Result<PairingStatus, String> {
    Ok(state.status().await)
}
#[tauri::command]
pub async fn mobile_pairing_start(
    state: tauri::State<'_, MobilePairing>,
    ip: String,
    port: u16,
) -> Result<PairingStatus, String> {
    state.start(ip, port).await
}
#[tauri::command]
pub async fn mobile_pairing_stop(
    state: tauri::State<'_, MobilePairing>,
) -> Result<PairingStatus, String> {
    Ok(state.stop().await)
}
#[tauri::command]
pub async fn mobile_pairing_invite(
    state: tauri::State<'_, MobilePairing>,
) -> Result<PairingStatus, String> {
    let running = state.running.lock().await;
    if !running.as_ref().is_some_and(|r| !r.task.is_finished()) {
        return Err("먼저 서버를 시작하세요.".into());
    }
    state.pairings.lock().invite(now_ms());
    drop(running);
    Ok(state.status().await)
}
#[tauri::command]
pub async fn mobile_pairing_approve(
    state: tauri::State<'_, MobilePairing>,
    request_id: String,
    approve: bool,
) -> Result<PairingStatus, String> {
    state
        .pairings
        .lock()
        .approve(&request_id, approve, now_ms())?;
    Ok(state.status().await)
}
#[tauri::command]
pub async fn mobile_pairing_revoke(
    state: tauri::State<'_, MobilePairing>,
    device_id: String,
) -> Result<PairingStatus, String> {
    state.pairings.lock().revoke(&device_id)?;
    Ok(state.status().await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invitation_is_consumed_before_approval_and_cannot_be_reused() {
        let mut state = Pairings::default();
        let invite = state.invite(1000);
        let pending = state.pair(&invite, "phone", 1001).unwrap();
        assert!(state.pair(&invite, "other", 1002).is_err());
        assert!(state.authenticate(&invite).is_none());
        let token = state
            .approve(&pending.request_id, true, 1003)
            .unwrap()
            .unwrap();
        assert!(state.authenticate(&token).is_some());
        assert!(state.approve(&pending.request_id, true, 1004).is_err());
    }

    #[test]
    fn expiration_and_rejection_never_issue_credentials() {
        let mut state = Pairings::default();
        let invite = state.invite(1000);
        assert!(state.pair(&invite, "phone", 121000).is_err());
        let invite = state.invite(200000);
        let pending = state.pair(&invite, "phone", 200001).unwrap();
        assert!(state.approve(&pending.request_id, true, 320000).is_err());
        let invite = state.invite(400000);
        let pending = state.pair(&invite, "phone", 400001).unwrap();
        assert!(state
            .approve(&pending.request_id, false, 400002)
            .unwrap()
            .is_none());
        assert!(state.devices.is_empty());
    }

    #[test]
    fn revocation_invalidates_token_and_signals_all_connections() {
        let mut state = Pairings::default();
        let invite = state.invite(0);
        let p = state.pair(&invite, "phone", 1).unwrap();
        let token = state.approve(&p.request_id, true, 2).unwrap().unwrap();
        let device = state.authenticate(&token).unwrap();
        let cancel = device.cancel.subscribe();
        let id = device.device_id.clone();
        state.revoke(&id).unwrap();
        assert!(*cancel.borrow());
        assert!(state.authenticate(&token).is_none());
    }

    #[test]
    fn address_and_origin_match_exact_bound_endpoint() {
        assert!(valid_headers(
            "127.0.0.1:80".parse().unwrap(),
            Some("127.0.0.1"),
            Some("http://127.0.0.1"),
            true
        ));
        let address: std::net::SocketAddr = "[fd7a:115c:a1e0::1]:43123".parse().unwrap();
        assert_eq!(base_url(address), "http://[fd7a:115c:a1e0::1]:43123/");
        assert!(valid_headers(
            address,
            Some("[fd7a:115c:a1e0::1]:43123"),
            Some("http://[fd7a:115c:a1e0::1]:43123"),
            true
        ));
        assert!(!valid_headers(
            address,
            Some("evil.example"),
            Some("http://[fd7a:115c:a1e0::1]:43123"),
            true
        ));
        assert!(!valid_headers(
            address,
            Some("[fd7a:115c:a1e0::1]:43123"),
            None,
            true
        ));
        assert!(!valid_headers(
            address,
            Some("[fd7a:115c:a1e0::1]:43123"),
            Some("null"),
            false
        ));
    }
}
