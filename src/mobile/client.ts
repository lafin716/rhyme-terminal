import { decodeBase64, encodeUtf8 } from "./encoding";
import type { AttachResult, ClientState, SessionInfo } from "./types";

const TOKEN_KEY = "winmux:mobile-device-token:v1";
const REQUEST_TIMEOUT_MS = 10_000;
const MAX_INPUT_BYTES = 16 * 1024;
const MAX_BUFFERED_OUTPUT_BYTES = 1024 * 1024;
const RECONNECT_DELAYS = [500, 1_000, 2_000, 4_000, 8_000] as const;

export interface SocketLike {
  readonly readyState: number;
  send(data: string): void;
  close(): void;
  onopen: ((event: any) => any) | null;
  onmessage: ((event: any) => any) | null;
  onclose: ((event: any) => any) | null;
  onerror: ((event: any) => any) | null;
}

interface StorageLike {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

interface PendingRequest {
  generation: number;
  resolve: (value: unknown) => void;
  reject: (error: Error) => void;
  timer: ReturnType<typeof setTimeout>;
}

interface ServerMessage {
  type: "pending" | "authenticated" | "result" | "error" | "event";
  verification?: string;
  token?: string;
  requestId?: number;
  message?: string;
  data?: unknown;
}

export interface MobileClientOptions {
  url: string;
  invite: string | null;
  deviceName: string;
  storage: StorageLike;
  createSocket: (url: string) => SocketLike;
  onState: (state: ClientState) => void;
  onOutput: (data: Uint8Array) => void;
  onAttach: (result: AttachResult, isCurrent: () => boolean) => void | Promise<void>;
  setTimer?: typeof setTimeout;
  clearTimer?: typeof clearTimeout;
}

export class MobileClient {
  private socket: SocketLike | null = null;
  private generation = 0;
  private nextRequestId = 1;
  private requests = new Map<number, PendingRequest>();
  private reconnectAttempt = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private listRefreshTimer: ReturnType<typeof setTimeout> | null = null;
  private stopped = false;
  private authenticated = false;
  private invite: string | null;
  private desiredSessionId: string | null = null;
  private applyingSnapshot = false;
  private bufferedOutput: Uint8Array[] = [];
  private bufferedOutputBytes = 0;
  private snapshotSequence = 0;
  private state: ClientState = {
    phase: "disconnected", verification: null, error: null, sessions: [], attached: null,
  };
  private readonly setTimer: typeof setTimeout;
  private readonly clearTimer: typeof clearTimeout;

  constructor(private readonly options: MobileClientOptions) {
    this.invite = options.invite;
    this.setTimer = options.setTimer ?? ((handler, timeout, ...args) => globalThis.setTimeout(handler, timeout, ...args));
    this.clearTimer = options.clearTimer ?? (timer => globalThis.clearTimeout(timer));
  }

  connect(): void {
    this.stopped = false;
    if (this.invite) this.options.storage.removeItem(TOKEN_KEY);
    this.open(false);
  }

  disconnect(): void {
    this.stopped = true;
    this.generation += 1;
    this.authenticated = false;
    this.applyingSnapshot = false;
    this.bufferedOutput = [];
    this.bufferedOutputBytes = 0;
    this.snapshotSequence += 1;
    if (this.reconnectTimer) this.clearTimer(this.reconnectTimer);
    if (this.listRefreshTimer) this.clearTimer(this.listRefreshTimer);
    this.reconnectTimer = null;
    this.listRefreshTimer = null;
    this.rejectRequests("연결이 종료되었습니다.");
    this.socket?.close();
    this.socket = null;
    this.update({ phase: "disconnected", attached: null });
  }

  async refreshSessions(): Promise<void> {
    const sessions = await this.request<SessionInfo[]>({ type: "list" });
    this.update({ sessions });
  }

  async attach(id: string): Promise<AttachResult> {
    const generation = this.generation;
    const previous = this.state.attached?.id;
    this.desiredSessionId = id;
    if (previous && previous !== id) {
      await this.request({ type: "detach", id: previous });
      if (this.desiredSessionId !== id) throw new Error("다른 터미널로 전환되었습니다.");
    }
    const result = await this.request<AttachResult>({ type: "attach", id });
    if (this.generation === generation && this.desiredSessionId === id) {
      await this.applyAttach(result);
      if (this.generation === generation && this.desiredSessionId === id) this.update({ attached: result.info });
    }
    return result;
  }

  async sendInput(value: string): Promise<void> {
    if (!this.authenticated || !this.state.attached) throw new Error("터미널이 연결되어 있지 않습니다.");
    const { bytes, base64 } = encodeUtf8(value);
    if (bytes.length === 0) return;
    if (bytes.length > MAX_INPUT_BYTES) throw new Error("입력은 UTF-8 기준 16KB 이하여야 합니다.");
    await this.request({ type: "input", id: this.state.attached.id, data: base64 });
  }

  private open(reconnecting: boolean): void {
    const generation = ++this.generation;
    this.authenticated = false;
    this.update({ phase: reconnecting ? "reconnecting" : "connecting", error: null, attached: null });
    const socket = this.options.createSocket(this.options.url);
    this.socket = socket;
    socket.onopen = () => {
      if (!this.isCurrent(socket, generation)) return;
      const token = this.options.storage.getItem(TOKEN_KEY);
      if (this.invite) socket.send(JSON.stringify({ type: "pair", invite: this.invite, name: [...this.options.deviceName].slice(0, 64).join("") }));
      else if (token) socket.send(JSON.stringify({ type: "auth", token }));
      else {
        this.update({ phase: "error", error: "초대 링크가 없거나 이 탭의 기기 인증이 만료되었습니다." });
        socket.close();
      }
    };
    socket.onmessage = event => {
      if (!this.isCurrent(socket, generation) || typeof event.data !== "string") return;
      this.handleMessage(event.data, generation);
    };
    socket.onerror = () => {
      if (this.isCurrent(socket, generation)) this.update({ error: "모바일 연결에 실패했습니다." });
    };
    socket.onclose = () => {
      if (!this.isCurrent(socket, generation)) return;
      this.socket = null;
      this.authenticated = false;
      this.rejectRequests("연결이 끊겼습니다.", generation);
      this.update({ attached: null });
      if (!this.stopped && this.options.storage.getItem(TOKEN_KEY)) this.scheduleReconnect();
      else if (!this.stopped && this.state.phase !== "error") this.update({ phase: "disconnected" });
    };
  }

  private handleMessage(raw: string, generation: number): void {
    let message: ServerMessage;
    try { message = JSON.parse(raw) as ServerMessage; }
    catch { this.failSocket("서버가 올바르지 않은 메시지를 보냈습니다."); return; }
    if (message.type === "pending" && message.verification) {
      this.invite = null;
      this.update({ phase: "pending", verification: message.verification, error: null });
      return;
    }
    if (message.type === "authenticated" && message.token) {
      this.options.storage.setItem(TOKEN_KEY, message.token);
      this.invite = null;
      this.authenticated = true;
      this.reconnectAttempt = 0;
      this.update({ phase: "authenticated", verification: null, error: null });
      void this.restoreAfterAuthentication(generation);
      return;
    }
    if ((message.type === "result" || message.type === "error") && typeof message.requestId === "number") {
      const pending = this.requests.get(message.requestId);
      if (!pending || pending.generation !== generation) return;
      this.requests.delete(message.requestId);
      this.clearTimer(pending.timer);
      if (message.type === "error") pending.reject(new Error(message.message ?? "요청에 실패했습니다."));
      else pending.resolve(message.data);
      return;
    }
    if (message.type === "event") this.handleEvent(message.data);
    else if (message.type === "error") {
      if (/인증|기기|token/i.test(message.message ?? "")) this.options.storage.removeItem(TOKEN_KEY);
      this.update({ phase: "error", error: message.message ?? "연결이 종료되었습니다." });
      this.socket?.close();
    }
  }

  private handleEvent(data: unknown): void {
    if (!data || typeof data !== "object") return;
    const event = data as { event?: string; id?: string; data?: string };
    const acceptsOutput = event.id === this.state.attached?.id
      || (this.applyingSnapshot && event.id === this.desiredSessionId);
    if (event.event === "pty_output" && acceptsOutput && event.data) {
      try {
        const output = decodeBase64(event.data);
        if (this.applyingSnapshot) {
          this.bufferedOutputBytes += output.byteLength;
          if (this.bufferedOutputBytes > MAX_BUFFERED_OUTPUT_BYTES) {
            this.failSocket("터미널 출력이 너무 빨라 다시 동기화합니다.");
            return;
          }
          this.bufferedOutput.push(output);
        }
        else this.options.onOutput(output);
      }
      catch { this.failSocket("터미널 출력 데이터가 손상되었습니다."); }
    } else if (["session_added", "session_removed", "session_renamed"].includes(event.event ?? "")) {
      if (this.listRefreshTimer) return;
      this.listRefreshTimer = this.setTimer(() => {
        this.listRefreshTimer = null;
        void this.refreshSessions().catch(error => this.update({ error: this.errorMessage(error) }));
      }, 100);
    } else if (event.event === "pty_exit" && acceptsOutput) {
      this.desiredSessionId = null;
      this.snapshotSequence += 1;
      this.applyingSnapshot = false;
      this.bufferedOutput = [];
      this.bufferedOutputBytes = 0;
      this.update({ attached: null });
    }
  }

  private async restoreAfterAuthentication(generation: number): Promise<void> {
    try {
      await this.refreshSessions();
      if (generation !== this.generation || !this.desiredSessionId) return;
      const result = await this.request<AttachResult>({ type: "attach", id: this.desiredSessionId });
      if (generation === this.generation && this.desiredSessionId === result.info.id) {
        await this.applyAttach(result);
        if (generation === this.generation && this.desiredSessionId === result.info.id) this.update({ attached: result.info });
      }
    } catch (error) {
      if (generation === this.generation) this.update({ error: this.errorMessage(error) });
    }
  }

  private request<T = unknown>(command: Record<string, unknown>): Promise<T> {
    const socket = this.socket;
    const generation = this.generation;
    if (!socket || socket.readyState !== 1 || !this.authenticated) return Promise.reject(new Error("연결되어 있지 않습니다."));
    const requestId = this.nextRequestId++;
    return new Promise<T>((resolve, reject) => {
      const timer = this.setTimer(() => {
        this.requests.delete(requestId);
        reject(new Error("요청 응답 시간이 초과되었습니다."));
        this.failSocket("요청 응답 시간이 초과되어 다시 연결합니다.");
      }, REQUEST_TIMEOUT_MS);
      this.requests.set(requestId, { generation, resolve: value => resolve(value as T), reject, timer });
      try { socket.send(JSON.stringify({ ...command, requestId })); }
      catch (error) {
        this.requests.delete(requestId);
        this.clearTimer(timer);
        reject(error instanceof Error ? error : new Error("요청 전송에 실패했습니다."));
      }
    });
  }

  private scheduleReconnect(): void {
    if (this.reconnectAttempt >= RECONNECT_DELAYS.length) {
      this.update({ phase: "error", error: "재연결에 실패했습니다. 페이지를 새로 고쳐 주세요." });
      return;
    }
    const delay = RECONNECT_DELAYS[this.reconnectAttempt++];
    this.update({ phase: "reconnecting" });
    this.reconnectTimer = this.setTimer(() => { this.reconnectTimer = null; this.open(true); }, delay);
  }

  private async applyAttach(result: AttachResult): Promise<void> {
    const generation = this.generation;
    const sequence = ++this.snapshotSequence;
    const isCurrent = () => generation === this.generation
      && sequence === this.snapshotSequence
      && result.info.id === this.desiredSessionId;
    this.applyingSnapshot = true;
    this.bufferedOutput = [];
    this.bufferedOutputBytes = 0;
    try {
      await this.options.onAttach(result, isCurrent);
      if (isCurrent()) for (const output of this.bufferedOutput) this.options.onOutput(output);
    } finally {
      if (sequence === this.snapshotSequence) {
        this.bufferedOutput = [];
        this.bufferedOutputBytes = 0;
        this.applyingSnapshot = false;
      }
    }
  }

  private rejectRequests(message: string, generation?: number): void {
    for (const [id, pending] of this.requests) {
      if (generation !== undefined && pending.generation !== generation) continue;
      this.clearTimer(pending.timer);
      pending.reject(new Error(message));
      this.requests.delete(id);
    }
  }

  private failSocket(message: string): void {
    this.update({ phase: "error", error: message });
    this.socket?.close();
  }

  private isCurrent(socket: SocketLike, generation: number): boolean {
    return this.socket === socket && this.generation === generation;
  }

  private update(patch: Partial<ClientState>): void {
    this.state = { ...this.state, ...patch };
    this.options.onState({ ...this.state, sessions: [...this.state.sessions] });
  }

  private errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : "요청에 실패했습니다.";
  }
}

export { MAX_INPUT_BYTES, TOKEN_KEY };
