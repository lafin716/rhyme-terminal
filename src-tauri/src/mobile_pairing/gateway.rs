use super::*;
use crate::ipc::{
    client::DaemonClient,
    protocol::{Event, Method},
};
use axum::{
    body::Body,
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    http::{HeaderMap, Request, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tower::ServiceExt;

include!(concat!(env!("OUT_DIR"), "/mobile_assets.rs"));
type ConnectDaemon = Arc<
    dyn Fn() -> std::pin::Pin<
            Box<dyn std::future::Future<Output = anyhow::Result<Arc<DaemonClient>>> + Send>,
        > + Send
        + Sync,
>;
const MESSAGE_LIMIT: usize = 24 * 1024;
const SEND_TIMEOUT: Duration = Duration::from_secs(5);
#[derive(Clone)]
struct Gateway {
    address: SocketAddr,
    pairings: Arc<Mutex<Pairings>>,
    stop: watch::Receiver<bool>,
    slots: Arc<Semaphore>,
    attempts: Arc<Mutex<(Instant, u32)>>,
    connect_daemon: ConnectDaemon,
}
pub(super) async fn serve(
    listener: tokio::net::TcpListener,
    address: SocketAddr,
    pairings: Arc<Mutex<Pairings>>,
    stop: watch::Receiver<bool>,
) {
    serve_with_connector(
        listener,
        address,
        pairings,
        stop,
        Arc::new(|| Box::pin(DaemonClient::connect_mobile())),
    )
    .await;
}
async fn serve_with_connector(
    listener: tokio::net::TcpListener,
    address: SocketAddr,
    pairings: Arc<Mutex<Pairings>>,
    mut stop: watch::Receiver<bool>,
    connect_daemon: ConnectDaemon,
) {
    let state = Gateway {
        address,
        pairings: pairings.clone(),
        stop: stop.clone(),
        slots: Arc::new(Semaphore::new(16)),
        attempts: Arc::new(Mutex::new((Instant::now(), 0))),
        connect_daemon,
    };
    let app = Router::new()
        .route("/ws", get(upgrade))
        .fallback(get(asset))
        .with_state(state);
    let mut tasks = tokio::task::JoinSet::new();
    loop {
        tokio::select! {
            biased;
            _=stop.changed()=>break,
            Some(_)=tasks.join_next(), if !tasks.is_empty()=>{},
            accepted=listener.accept()=>{
                let Ok((stream,_))=accepted else {break};
                if tasks.len()>=32 { drop(stream); continue; }
                let router=app.clone();
                tasks.spawn(async move {
                    let service=hyper::service::service_fn(move |request:Request<hyper::body::Incoming>| { let app=router.clone(); async move { app.oneshot(request.map(Body::new)).await } });
                    let mut builder=hyper::server::conn::http1::Builder::new();
                    builder.timer(hyper_util::rt::TokioTimer::new()).header_read_timeout(Duration::from_secs(5)).max_buf_size(16*1024).keep_alive(true);
                    let connection=builder.serve_connection(hyper_util::rt::TokioIo::new(stream),service).with_upgrades();
                    let _=tokio::time::timeout(Duration::from_secs(10),connection).await;
                });
            }
        }
    }
    pairings.lock().clear();
    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
}
fn headers_ok(state: &Gateway, headers: &HeaderMap, ws: bool) -> bool {
    !headers.contains_key("transfer-encoding")
        && headers.get("content-length").is_none_or(|v| v == "0")
        && headers.get("origin").is_none_or(|v| v.to_str().is_ok())
        && headers.get_all("host").iter().count() == 1
        && headers.get_all("origin").iter().count() <= 1
        && valid_headers(
            state.address,
            headers.get("host").and_then(|v| v.to_str().ok()),
            headers.get("origin").and_then(|v| v.to_str().ok()),
            ws,
        )
}
async fn asset(State(state): State<Gateway>, headers: HeaderMap, uri: Uri) -> Response {
    if !headers_ok(&state, &headers, false) || uri.query().is_some() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let path = if uri.path() == "/" {
        "mobile.html"
    } else {
        uri.path().trim_start_matches('/')
    };
    let Some((_, bytes)) = MOBILE_ASSETS.iter().find(|(p, _)| *p == path) else {
        return if path == "mobile.html" {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "모바일 화면이 빌드되지 않았습니다. pnpm build 후 앱을 다시 빌드하세요.",
            )
                .into_response()
        } else {
            StatusCode::NOT_FOUND.into_response()
        };
    };
    let mime = if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".js") {
        "text/javascript; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".woff2") {
        "font/woff2"
    } else {
        "application/octet-stream"
    };
    ([("content-type",mime),("cache-control","no-store"),("x-content-type-options","nosniff"),("referrer-policy","no-referrer"),("content-security-policy","default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data:; frame-ancestors 'none'; base-uri 'none'; form-action 'none'")],*bytes).into_response()
}
async fn upgrade(
    State(state): State<Gateway>,
    headers: HeaderMap,
    uri: Uri,
    ws: WebSocketUpgrade,
) -> Response {
    if !headers_ok(&state, &headers, true) || uri.query().is_some() {
        return StatusCode::FORBIDDEN.into_response();
    }
    {
        let mut rate = state.attempts.lock();
        if rate.0.elapsed() >= Duration::from_secs(60) {
            *rate = (Instant::now(), 0);
        }
        if rate.1 >= 30 {
            return StatusCode::TOO_MANY_REQUESTS.into_response();
        }
        rate.1 += 1;
    }
    let Ok(permit) = state.slots.clone().try_acquire_owned() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    ws.max_message_size(MESSAGE_LIMIT)
        .max_frame_size(MESSAGE_LIMIT)
        .read_buffer_size(4096)
        .write_buffer_size(0)
        .max_write_buffer_size(2 * 1024 * 1024)
        .on_upgrade(move |socket| async move {
            let _permit = permit;
            socket_session(socket, state).await;
        })
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
enum Hello {
    Pair { invite: String, name: String },
    Auth { token: String },
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
enum Command {
    List {
        #[serde(rename = "requestId")]
        request_id: u64,
    },
    Attach {
        #[serde(rename = "requestId")]
        request_id: u64,
        id: Uuid,
    },
    Detach {
        #[serde(rename = "requestId")]
        request_id: u64,
        id: Uuid,
    },
    Input {
        #[serde(rename = "requestId")]
        request_id: u64,
        id: Uuid,
        data: String,
    },
}
async fn send(socket: &mut WebSocket, value: Value) -> Result<(), String> {
    tokio::time::timeout(
        SEND_TIMEOUT,
        socket.send(Message::Text(value.to_string().into())),
    )
    .await
    .map_err(|_| "수신 속도가 느려 연결을 종료합니다.".to_string())?
    .map_err(|_| "연결이 종료되었습니다.".into())
}
async fn read_json<T: serde::de::DeserializeOwned>(socket: &mut WebSocket) -> Result<T, String> {
    match socket.recv().await {
        Some(Ok(Message::Text(text))) => {
            serde_json::from_str(&text).map_err(|_| "잘못된 요청입니다.".into())
        }
        _ => Err("연결이 종료되었거나 잘못된 메시지입니다.".into()),
    }
}
struct Registration {
    pairings: Arc<Mutex<Pairings>>,
    token: String,
}
impl Drop for Registration {
    fn drop(&mut self) {
        if let Some(d) = self.pairings.lock().devices.get_mut(&self.token) {
            d.connections = d.connections.saturating_sub(1);
        }
    }
}
struct PendingGuard {
    pairings: Arc<Mutex<Pairings>>,
    id: String,
}
impl Drop for PendingGuard {
    fn drop(&mut self) {
        self.pairings.lock().pending.remove(&self.id);
    }
}
async fn authenticate(
    socket: &mut WebSocket,
    state: &Gateway,
) -> Result<(String, String, watch::Receiver<bool>, Registration), String> {
    let hello = tokio::time::timeout(Duration::from_secs(10), read_json::<Hello>(socket))
        .await
        .map_err(|_| "인증 시간이 초과되었습니다.".to_string())??;
    let token = match hello {
        Hello::Auth { token } => token,
        Hello::Pair { invite, name } => {
            let (pending, mut result, expires) = {
                let mut pairings = state.pairings.lock();
                let pending = pairings.pair(&invite, &name, now_ms())?;
                let p = &pairings.pending[&pending.request_id];
                (pending, p.result.subscribe(), p.expires)
            };
            let _guard = PendingGuard {
                pairings: state.pairings.clone(),
                id: pending.request_id,
            };
            send(
                socket,
                json!({"type":"pending","verification":pending.verification}),
            )
            .await?;
            let waiting = async {
                loop {
                    if let Some(result) = result.borrow().clone() {
                        return result;
                    }
                    tokio::select! {
                        changed=result.changed()=>if changed.is_err() {return Err("승인 요청이 종료되었습니다.".into());},
                        _=socket.recv()=>return Err("승인 대기 중 연결이 종료되었거나 요청을 보냈습니다.".into()),
                    }
                }
            };
            tokio::time::timeout(
                Duration::from_millis(expires.saturating_sub(now_ms())),
                waiting,
            )
            .await
            .map_err(|_| "승인 시간이 만료되었습니다.".to_string())??
        }
    };
    let (id, cancel) = {
        let mut p = state.pairings.lock();
        let d = p
            .devices
            .get_mut(&token)
            .ok_or("기기 인증에 실패했습니다. PC에서 다시 페어링하세요.")?;
        if d.connections >= 4 {
            return Err("이 기기의 동시 연결 수를 초과했습니다.".into());
        }
        d.connections += 1;
        (d.device_id.clone(), d.cancel.subscribe())
    };
    let registration = Registration {
        pairings: state.pairings.clone(),
        token: token.clone(),
    };
    Ok((token, id, cancel, registration))
}
async fn socket_session(mut socket: WebSocket, state: Gateway) {
    let mut stop = state.stop.clone();
    let auth = tokio::select! {biased; _=stop.changed()=>return,auth=authenticate(&mut socket,&state)=>auth};
    let (token, id, mut cancel, _registration) = match auth {
        Ok(value) => value,
        Err(message) => {
            let _ = send(&mut socket, json!({"type":"error","message":message})).await;
            return;
        }
    };
    if *cancel.borrow() {
        return;
    }
    let work = async {
        send(
            &mut socket,
            json!({"type":"authenticated","token":token,"deviceId":id}),
        )
        .await?;
        let daemon = (state.connect_daemon)()
            .await
            .map_err(|_| "터미널 서버에 연결할 수 없습니다.".to_string())?;
        let mut events = daemon.events();
        let mut attached = None;
        let mut rate = (Instant::now(), 0u32);
        loop {
            tokio::select! {
                _=daemon.disconnected()=>return Err("터미널 서버 연결이 종료되었습니다.".into()),
                command=read_json::<Command>(&mut socket)=>{
                    if rate.0.elapsed()>=Duration::from_secs(1) {rate=(Instant::now(),0);}
                    rate.1+=1; if rate.1>60 {return Err("요청이 너무 많습니다.".into());}
                    let command=command?;
                    let (request_id,method,next)=match command {
                        Command::List {request_id}=>(request_id,Method::ListSessions,attached),
                        Command::Attach {request_id,id}=>{
                            // A fresh receiver is installed before issuing atomic attach. The daemon sends its snapshot before live events.

                            (request_id,Method::AttachSessionAtomic {id},Some(id))
                        },
                        Command::Detach {request_id,id} if attached==Some(id)=>(request_id,Method::DetachSession {id},None),
                        Command::Input {request_id,id,data} if attached==Some(id)=>{
                            if base64::engine::general_purpose::STANDARD.decode(&data).map_or(true,|bytes|bytes.len()>16*1024 || std::str::from_utf8(&bytes).is_err()) {
                                send(&mut socket,json!({"type":"error","requestId":request_id,"message":"입력은 16KB 이하의 UTF-8이어야 합니다."})).await?; continue;
                            }
                            (request_id,Method::WriteSession {id,data},attached)
                        },
                        Command::Input {request_id,..}|Command::Detach {request_id,..}=>{send(&mut socket,json!({"type":"error","requestId":request_id,"message":"연결된 터미널이 아닙니다."})).await?;continue;}
                    };
                    let attaching=matches!(method,Method::AttachSessionAtomic {..});
                    match tokio::time::timeout(Duration::from_secs(10),daemon.request_with_events(method)).await {
                        Ok(Ok((data,receiver)))=>{if attaching {events=receiver;} attached=next;send(&mut socket,json!({"type":"result","requestId":request_id,"data":data})).await?;},
                        Ok(Err(error))=>{if attaching {attached=None;} send(&mut socket,json!({"type":"error","requestId":request_id,"message":format!("터미널 요청 실패: {error}. 연결 실패 시 daemon 업데이트가 필요할 수 있습니다.")})).await?;},
                        Err(_)=>return Err(if attaching {"연결 응답 시간이 초과되었습니다. daemon을 업데이트한 후 다시 시작하세요.".into()} else {"터미널 응답 시간이 초과되었습니다. 입력은 자동 재전송하지 않습니다.".into()}),
                    }
                },
                event=events.recv()=>{
                    let event=event.map_err(|_|"출력이 지연되었거나 터미널 연결이 종료되었습니다. 다시 연결하세요.".to_string())?;
                    let allowed=match &event {
                        Event::PtyOutput {id,..}|Event::PtyExit {id,..}=>attached==Some(*id),
                        Event::SessionAdded {..}|Event::SessionRemoved {..}|Event::SessionRenamed {..}=>true,
                        _=>false,
                    };
                    if !allowed {continue;}
                    send(&mut socket,json!({"type":"event","data":event})).await?;
                }
            }
        }
    };
    let result: Result<(), String> = tokio::select! {biased; _=stop.changed()=>Err("모바일 연결 서버가 중지되었습니다.".into()),_=cancel.changed()=>Err("기기 연결이 해제되었습니다.".into()),result=work=>result};
    if let Err(message) = result {
        let _ = send(&mut socket, json!({"type":"error","message":message})).await;
    }
    let _ = tokio::time::timeout(Duration::from_secs(1), socket.send(Message::Close(None))).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::{
        connect_async,
        tungstenite::{client::IntoClientRequest, Message as ClientMessage},
    };
    async fn running() -> (
        SocketAddr,
        Arc<Mutex<Pairings>>,
        watch::Sender<bool>,
        tokio::task::JoinHandle<()>,
    ) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let pairings = Arc::new(Mutex::new(Pairings::default()));
        let (stop, rx) = watch::channel(false);
        let task = tokio::spawn(serve(listener, address, pairings.clone(), rx));
        (address, pairings, stop, task)
    }
    async fn socket(
        address: SocketAddr,
    ) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>
    {
        let mut request = format!("ws://{address}/ws").into_client_request().unwrap();
        request
            .headers_mut()
            .insert("origin", format!("http://{address}").parse().unwrap());
        connect_async(request).await.unwrap().0
    }
    async fn response(
        socket: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) -> Value {
        let frame = tokio::time::timeout(Duration::from_secs(2), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        serde_json::from_str(frame.to_text().unwrap()).unwrap()
    }
    #[tokio::test]
    async fn http_host_origin_asset_and_unknown_path_policy() {
        let (address, _, stop, task) = running().await;
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let root = client.get(base_url(address)).send().await.unwrap();
        assert_eq!(
            root.status().as_u16(),
            if MOBILE_ASSETS.iter().any(|(path, _)| *path == "mobile.html") {
                200
            } else {
                503
            }
        );
        assert_eq!(
            client
                .get(base_url(address))
                .header("host", "evil.example")
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
        assert_eq!(
            client
                .get(base_url(address))
                .header("origin", "http://evil.example")
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
        assert_eq!(
            client
                .get(format!("{}missing.js", base_url(address)))
                .send()
                .await
                .unwrap()
                .status(),
            404
        );
        assert_eq!(
            client
                .get(format!("{}?invite=secret", base_url(address)))
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
        stop.send_replace(true);
        task.await.unwrap();
    }
    #[tokio::test]
    async fn websocket_requires_exact_origin_and_rejects_session_access_before_auth() {
        let (address, _, stop, task) = running().await;
        let request = format!("ws://{address}/ws").into_client_request().unwrap();
        assert!(connect_async(request).await.is_err());
        let mut ws = socket(address).await;
        ws.send(ClientMessage::Text(
            json!({"type":"list","requestId":1}).to_string().into(),
        ))
        .await
        .unwrap();
        let value = response(&mut ws).await;
        assert_eq!(value["type"], "error");
        assert!(value.get("data").is_none());
        let mut ws = socket(address).await;
        ws.send(ClientMessage::Text(
            json!({"type":"auth","token":"wrong"}).to_string().into(),
        ))
        .await
        .unwrap();
        assert_eq!(response(&mut ws).await["type"], "error");
        stop.send_replace(true);
        task.await.unwrap();
    }
    #[tokio::test]
    async fn pending_pairing_is_single_use_and_removed_when_socket_closes() {
        let (address, pairings, stop, task) = running().await;
        let invite = pairings.lock().invite(now_ms());
        let mut first = socket(address).await;
        first
            .send(ClientMessage::Text(
                json!({"type":"pair","invite":invite,"name":"테스트 기기"})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
        let pending = response(&mut first).await;
        assert_eq!(pending["type"], "pending");
        assert_eq!(pending["verification"].as_str().unwrap().len(), 6);
        assert_eq!(pairings.lock().pending.len(), 1);
        let mut second = socket(address).await;
        second
            .send(ClientMessage::Text(
                json!({"type":"pair","invite":invite,"name":"second"})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
        assert_eq!(response(&mut second).await["type"], "error");
        first.close(None).await.unwrap();
        drop(first);
        tokio::time::timeout(Duration::from_secs(2), async {
            while !pairings.lock().pending.is_empty() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert!(pairings.lock().devices.is_empty());
        stop.send_replace(true);
        task.await.unwrap();
    }
    #[tokio::test]
    async fn rejection_reaches_waiting_socket_and_server_stop_closes_pending_socket() {
        let (address, pairings, stop, task) = running().await;
        for reject in [true, false] {
            let invite = pairings.lock().invite(now_ms());
            let mut ws = socket(address).await;
            ws.send(ClientMessage::Text(
                json!({"type":"pair","invite":invite,"name":"phone"})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
            assert_eq!(response(&mut ws).await["type"], "pending");
            if reject {
                let id = pairings.lock().pending.keys().next().unwrap().clone();
                pairings.lock().approve(&id, false, now_ms()).unwrap();
            } else {
                stop.send_replace(true);
            }
            let frame = tokio::time::timeout(Duration::from_secs(2), ws.next())
                .await
                .unwrap();
            if reject {
                assert_eq!(
                    serde_json::from_str::<Value>(frame.unwrap().unwrap().to_text().unwrap())
                        .unwrap()["type"],
                    "error"
                );
            }
        }
        task.await.unwrap();
        assert!(pairings.lock().pending.is_empty());
    }
    #[test]
    fn protocol_whitelist_disallows_resize_kill_create_and_extra_fields() {
        for value in [
            json!({"type":"resize","requestId":1,"cols":20}),
            json!({"type":"kill","requestId":1}),
            json!({"type":"create","requestId":1}),
            json!({"type":"list","requestId":1,"method":"kill_server"}),
        ] {
            assert!(serde_json::from_value::<Command>(value).is_err());
        }
    }
}

#[cfg(all(test, windows))]
mod positive_integration {
    use super::*;
    use crate::ipc::{
        protocol::{Request as PipeRequest, ServerMsg},
        read_frame, write_frame,
    };
    use futures_util::{SinkExt, StreamExt};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};
    use tokio_tungstenite::{
        connect_async,
        tungstenite::{client::IntoClientRequest, Message as WireMessage},
    };
    type Socket = tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >;
    #[derive(Default)]
    struct Observations {
        opened: AtomicUsize,
        active: AtomicUsize,
        methods: Mutex<Vec<String>>,
        input: Mutex<Vec<Vec<u8>>>,
    }
    struct Active(Arc<Observations>);
    impl Drop for Active {
        fn drop(&mut self) {
            self.0.active.fetch_sub(1, Ordering::SeqCst);
        }
    }
    fn fixture_connector(observed: Arc<Observations>, session: Uuid) -> ConnectDaemon {
        Arc::new(move || {
            let observed = observed.clone();
            Box::pin(async move {
                let name = format!(r"\\.\pipe\winmux-mobile-integration-{}", Uuid::new_v4());
                let mut server = ServerOptions::new()
                    .first_pipe_instance(true)
                    .create(&name)?;
                let pipe = ClientOptions::new().open(&name)?;
                server.connect().await?;
                observed.opened.fetch_add(1, Ordering::SeqCst);
                observed.active.fetch_add(1, Ordering::SeqCst);
                tokio::spawn(async move {
                    let _active = Active(observed.clone());
                    let info = json!({"id":session,"name":"w1.integration","shell":"fixture","cwd":null,"cols":120,"rows":30,"agent":"terminal"});
                    while let Ok(frame) = read_frame(&mut server).await {
                        let request: PipeRequest = serde_json::from_slice(&frame).unwrap();
                        let (label, data, live) = match request.method {
                            Method::ListSessions => ("list", json!([info.clone()]), false),
                            Method::AttachSessionAtomic { id } if id == session => (
                                "attach",
                                json!({"info":info,"scrollback":"c25hcHNob3Q="}),
                                true,
                            ),
                            Method::WriteSession { id, data } if id == session => {
                                observed.input.lock().push(
                                    base64::engine::general_purpose::STANDARD
                                        .decode(data)
                                        .unwrap(),
                                );
                                ("input", Value::Null, false)
                            }
                            Method::DetachSession { id } if id == session => {
                                ("detach", Value::Null, false)
                            }
                            _ => ("FORBIDDEN", Value::Null, false),
                        };
                        observed.methods.lock().push(label.into());
                        let result = ServerMsg::Response {
                            id: request.id,
                            result: data,
                        };
                        if write_frame(&mut server, &serde_json::to_vec(&result).unwrap())
                            .await
                            .is_err()
                        {
                            break;
                        }
                        if live {
                            let event = ServerMsg::Event(Event::PtyOutput {
                                id: session,
                                data: "bGl2ZQ==".into(),
                            });
                            if write_frame(&mut server, &serde_json::to_vec(&event).unwrap())
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                });
                DaemonClient::from_pipe_capacity(pipe, 64).await
            })
        })
    }
    async fn connect(address: SocketAddr) -> Socket {
        let mut request = format!("ws://{address}/ws").into_client_request().unwrap();
        request
            .headers_mut()
            .insert("origin", format!("http://{address}").parse().unwrap());
        connect_async(request).await.unwrap().0
    }
    async fn send(socket: &mut Socket, value: Value) {
        socket
            .send(WireMessage::Text(value.to_string().into()))
            .await
            .unwrap();
    }
    async fn receive(socket: &mut Socket) -> Value {
        let message = tokio::time::timeout(Duration::from_secs(3), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        serde_json::from_str(message.to_text().unwrap()).unwrap()
    }
    async fn wait_active(observed: &Observations, count: usize) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while observed.active.load(Ordering::SeqCst) != count {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
    }
    async fn pair(address: SocketAddr, pairings: &Mutex<Pairings>) -> (Socket, Value) {
        let invite = pairings.lock().invite(now_ms());
        let mut socket = connect(address).await;
        send(
            &mut socket,
            json!({"type":"pair","invite":invite,"name":"테스트 휴대폰"}),
        )
        .await;
        let pending = receive(&mut socket).await;
        assert_eq!(pending["type"], "pending");
        let request = pairings
            .lock()
            .pending
            .values()
            .next()
            .unwrap()
            .view
            .clone();
        assert_eq!(pending["verification"], request.verification);
        pairings
            .lock()
            .approve(&request.request_id, true, now_ms())
            .unwrap();
        let authenticated = receive(&mut socket).await;
        assert_eq!(authenticated["type"], "authenticated");
        (socket, authenticated)
    }
    #[tokio::test]
    async fn approved_websocket_lists_attaches_inputs_reauthenticates_and_revokes_on_isolated_pipes(
    ) {
        let session = Uuid::new_v4();
        let observed = Arc::new(Observations::default());
        let pairings = Arc::new(Mutex::new(Pairings::default()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stop, rx) = watch::channel(false);
        let server = tokio::spawn(serve_with_connector(
            listener,
            address,
            pairings.clone(),
            rx,
            fixture_connector(observed.clone(), session),
        ));
        let (mut socket, credentials) = pair(address, &pairings).await;
        wait_active(&observed, 1).await;
        send(&mut socket, json!({"type":"list","requestId":1})).await;
        let result = receive(&mut socket).await;
        assert_eq!(result["requestId"], 1);
        assert_eq!(result["data"][0]["id"], session.to_string());
        send(
            &mut socket,
            json!({"type":"attach","requestId":2,"id":session}),
        )
        .await;
        let snapshot = receive(&mut socket).await;
        assert_eq!(snapshot["type"], "result");
        assert_eq!(snapshot["data"]["scrollback"], "c25hcHNob3Q=");
        assert_eq!(snapshot["data"]["info"]["cols"], 120);
        assert_eq!(snapshot["data"]["info"]["rows"], 30);
        let live = receive(&mut socket).await;
        assert_eq!(live["type"], "event");
        assert_eq!(live["data"]["data"], "bGl2ZQ==");
        let bytes = "한글 입력\r\t\u{1b}\u{3}".as_bytes();
        send(&mut socket,json!({"type":"input","requestId":3,"id":session,"data":base64::engine::general_purpose::STANDARD.encode(bytes)})).await;
        assert_eq!(
            receive(&mut socket).await,
            json!({"type":"result","requestId":3,"data":null})
        );
        assert_eq!(*observed.input.lock(), vec![bytes.to_vec()]);
        socket.close(None).await.unwrap();
        drop(socket);
        wait_active(&observed, 0).await;
        let mut resumed = connect(address).await;
        send(
            &mut resumed,
            json!({"type":"auth","token":credentials["token"]}),
        )
        .await;
        assert_eq!(receive(&mut resumed).await, credentials);
        wait_active(&observed, 1).await;
        assert_eq!(observed.opened.load(Ordering::SeqCst), 2);
        send(
            &mut resumed,
            json!({"type":"input","requestId":4,"id":session,"data":"eA=="}),
        )
        .await;
        assert_eq!(receive(&mut resumed).await["type"], "error"); // Re-auth does not implicitly attach or replay input.
        send(
            &mut resumed,
            json!({"type":"attach","requestId":5,"id":session}),
        )
        .await;
        assert_eq!(receive(&mut resumed).await["type"], "result");
        assert_eq!(receive(&mut resumed).await["type"], "event");
        pairings
            .lock()
            .revoke(credentials["deviceId"].as_str().unwrap())
            .unwrap();
        let _ = resumed
            .send(WireMessage::Text(
                json!({"type":"input","requestId":6,"id":session,"data":"eA=="})
                    .to_string()
                    .into(),
            ))
            .await;
        // Sending after revocation may race the TCP close and produce a reset rather than the best-effort error frame.
        match tokio::time::timeout(Duration::from_secs(3), resumed.next())
            .await
            .unwrap()
        {
            Some(Ok(WireMessage::Text(text))) => assert_eq!(
                serde_json::from_str::<Value>(&text).unwrap()["type"],
                "error"
            ),
            Some(Ok(WireMessage::Close(_))) | Some(Err(_)) | None => {}
            other => panic!("unexpected post-revocation payload: {other:?}"),
        }
        wait_active(&observed, 0).await;
        let mut denied = connect(address).await;
        send(
            &mut denied,
            json!({"type":"auth","token":credentials["token"]}),
        )
        .await;
        assert_eq!(receive(&mut denied).await["type"], "error");
        assert_eq!(observed.opened.load(Ordering::SeqCst), 2);
        let (mut stopped, _) = pair(address, &pairings).await;
        wait_active(&observed, 1).await;
        stop.send_replace(true);
        assert_eq!(receive(&mut stopped).await["type"], "error");
        wait_active(&observed, 0).await;
        server.await.unwrap();
        assert!(pairings.lock().devices.is_empty());
        assert_eq!(
            *observed.methods.lock(),
            vec!["list", "attach", "input", "attach"]
        );
        assert_eq!(*observed.input.lock(), vec![bytes.to_vec()]);
    }

    #[tokio::test]
    async fn built_mobile_html_and_script_styles_are_served_with_correct_mime() {
        if !MOBILE_ASSETS.iter().any(|(path, _)| *path == "mobile.html") {
            return;
        } // Direct Rust builds intentionally support explicit 503 fallback.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stop, rx) = watch::channel(false);
        let server = tokio::spawn(serve(
            listener,
            address,
            Arc::new(Mutex::new(Pairings::default())),
            rx,
        ));
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let response = client.get(base_url(address)).send().await.unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(
            response.headers()["content-type"],
            "text/html; charset=utf-8"
        );
        let html = response.text().await.unwrap();
        assert!(html.contains("assets/"));
        for extension in [".js", ".css"] {
            let (path, expected) = MOBILE_ASSETS
                .iter()
                .find(|(path, _)| path.ends_with(extension))
                .unwrap();
            let response = client
                .get(format!("{}{path}", base_url(address)))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), 200);
            assert_eq!(
                response.headers()["content-type"],
                if extension == ".js" {
                    "text/javascript; charset=utf-8"
                } else {
                    "text/css; charset=utf-8"
                }
            );
            assert_eq!(response.bytes().await.unwrap().as_ref(), *expected);
        }
        stop.send_replace(true);
        server.await.unwrap();
    }
}
