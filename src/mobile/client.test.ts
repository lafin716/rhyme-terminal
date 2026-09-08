import { afterEach, describe, expect, it, vi } from "vitest";
import { MobileClient, MAX_INPUT_BYTES, TOKEN_KEY, type SocketLike } from "./client";
import { takeInvitation } from "./invitation";
import type { AttachResult, ClientState } from "./types";

class FakeSocket implements SocketLike {
  readyState = 0;
  sent: string[] = [];
  onopen: (() => void) | null = null;
  onmessage: ((event: { data: string }) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  send(data: string) { this.sent.push(data); }
  close() { this.readyState = 3; this.onclose?.(); }
  open() { this.readyState = 1; this.onopen?.(); }
  receive(message: unknown) { this.onmessage?.({ data: JSON.stringify(message) }); }
  drop() { this.readyState = 3; this.onclose?.(); }
}

function setup(invite: string | null = null, onAttach?: (result: AttachResult, isCurrent: () => boolean) => Promise<void>) {
  const values = new Map<string, string>();
  const sockets: FakeSocket[] = [];
  const states: ClientState[] = [];
  const attachments: AttachResult[] = [];
  const output = vi.fn();
  const client = new MobileClient({
    url: "ws://127.0.0.1:43123/ws",
    invite,
    deviceName: "테스트 기기",
    storage: {
      getItem: key => values.get(key) ?? null,
      setItem: (key, value) => { values.set(key, value); },
      removeItem: key => { values.delete(key); },
    },
    createSocket: () => { const socket = new FakeSocket(); sockets.push(socket); return socket; },
    onState: state => states.push(state),
    onOutput: output,
    onAttach: onAttach ?? (result => { attachments.push(result); }),
  });
  return { client, sockets, states, values, attachments, output };
}

function authenticate(context: ReturnType<typeof setup>, token = "device-token") {
  context.client.connect();
  const socket = context.sockets[0];
  socket.open();
  socket.receive({ type: "authenticated", token, deviceId: "device" });
  const list = JSON.parse(socket.sent[socket.sent.length - 1]);
  socket.receive({ type: "result", requestId: list.requestId, data: [] });
  return socket;
}

afterEach(() => vi.useRealTimers());

describe("mobile invitation", () => {
  it("extracts the invite and removes the entire fragment from browser history", () => {
    const replace = vi.fn();
    expect(takeInvitation({ hash: "#invite=secret", pathname: "/", search: "" }, replace)).toBe("secret");
    expect(replace).toHaveBeenCalledWith("/");
  });
});

describe("MobileClient", () => {
  it("prefers a new invite over a stale tab token and exposes pending verification", () => {
    const context = setup("new-invite");
    context.values.set(TOKEN_KEY, "old-token");
    context.client.connect();
    context.sockets[0].open();
    expect(JSON.parse(context.sockets[0].sent[0])).toEqual({ type: "pair", invite: "new-invite", name: "테스트 기기" });
    expect(context.values.has(TOKEN_KEY)).toBe(false);
    context.sockets[0].receive({ type: "pending", verification: "123456" });
    expect(context.states[context.states.length - 1]).toMatchObject({ phase: "pending", verification: "123456" });
  });

  it("stores authentication, lists sessions, attaches with snapshot, and switches by detaching first", async () => {
    const context = setup();
    context.values.set(TOKEN_KEY, "saved");
    const socket = authenticate(context, "renewed");
    await Promise.resolve();
    expect(context.values.get(TOKEN_KEY)).toBe("renewed");
    const first = context.client.attach("one");
    const attachOne = JSON.parse(socket.sent[socket.sent.length - 1]);
    const firstResult = { info: { id: "one", name: "one", shell: "pwsh", cwd: null, cols: 120, rows: 30 }, scrollback: "" };
    socket.receive({ type: "result", requestId: attachOne.requestId, data: firstResult });
    await first;
    expect(context.attachments).toEqual([firstResult]);
    const second = context.client.attach("two");
    const detach = JSON.parse(socket.sent[socket.sent.length - 1]);
    expect(detach).toMatchObject({ type: "detach", id: "one" });
    socket.receive({ type: "result", requestId: detach.requestId, data: null });
    await Promise.resolve();
    const attachTwo = JSON.parse(socket.sent[socket.sent.length - 1]);
    expect(attachTwo).toMatchObject({ type: "attach", id: "two" });
    const secondResult = { ...firstResult, info: { ...firstResult.info, id: "two", name: "two" } };
    socket.receive({ type: "result", requestId: attachTwo.requestId, data: secondResult });
    await second;
  });

  it("rejects oversized UTF-8 input before sending", async () => {
    const context = setup();
    context.values.set(TOKEN_KEY, "saved");
    const socket = authenticate(context);
    await Promise.resolve();
    const attached = context.client.attach("one");
    const request = JSON.parse(socket.sent[socket.sent.length - 1]);
    socket.receive({ type: "result", requestId: request.requestId, data: { info: { id: "one", name: "one", shell: "pwsh", cwd: null, cols: 80, rows: 24 }, scrollback: "" } });
    await attached;
    const count = socket.sent.length;
    await expect(context.client.sendInput("가".repeat(Math.ceil(MAX_INPUT_BYTES / 3) + 1))).rejects.toThrow("16KB");
    expect(socket.sent).toHaveLength(count);
  });

  it("does not replay uncertain input and ignores callbacks from a stale socket", async () => {
    vi.useFakeTimers();
    const context = setup();
    context.values.set(TOKEN_KEY, "saved");
    const first = authenticate(context);
    await Promise.resolve();
    const attaching = context.client.attach("one");
    const attachRequest = JSON.parse(first.sent[first.sent.length - 1]);
    first.receive({ type: "result", requestId: attachRequest.requestId, data: { info: { id: "one", name: "one", shell: "pwsh", cwd: null, cols: 80, rows: 24 }, scrollback: "" } });
    await attaching;
    const input = context.client.sendInput("echo once\r");
    const sentInput = JSON.parse(first.sent[first.sent.length - 1]);
    first.drop();
    await expect(input).rejects.toThrow("연결이 끊겼습니다");
    await vi.advanceTimersByTimeAsync(500);
    const second = context.sockets[1];
    second.open();
    expect(second.sent).toHaveLength(1);
    expect(JSON.parse(second.sent[0])).toEqual({ type: "auth", token: "device-token" });
    first.receive({ type: "result", requestId: sentInput.requestId, data: null });
    expect(context.states[context.states.length - 1]?.phase).toBe("reconnecting");
  });
});

it.each([false, true])("does not revive a terminated session while asynchronous snapshot applies (reconnect=%s)", async reconnect => {
  vi.useFakeTimers();
  let pause = !reconnect;
  let release!: () => void;
  let current: () => boolean = () => true;
  const context = setup(null, async (_result, isCurrent) => {
    current = isCurrent;
    if (pause) await new Promise<void>(resolve => { release = resolve; });
  });
  context.values.set(TOKEN_KEY, "saved");
  let socket = authenticate(context);
  await Promise.resolve();
  const result = { info: { id: "one", name: "one", shell: "pwsh", cwd: null, cols: 80, rows: 24 }, scrollback: "" };
  const attaching = context.client.attach("one");
  const initial = JSON.parse(socket.sent[socket.sent.length - 1]);
  socket.receive({ type: "result", requestId: initial.requestId, data: result });
  await Promise.resolve();
  if (reconnect) {
    await attaching;
    pause = true;
    socket.drop();
    await vi.advanceTimersByTimeAsync(500);
    socket = context.sockets[1]; socket.open();
    socket.receive({ type: "authenticated", token: "device-token", deviceId: "device" });
    const list = JSON.parse(socket.sent[socket.sent.length - 1]);
    socket.receive({ type: "result", requestId: list.requestId, data: [result.info] });
    await vi.advanceTimersByTimeAsync(0);
    const request = JSON.parse(socket.sent[socket.sent.length - 1]);
    expect(request.type).toBe("attach");
    socket.receive({ type: "result", requestId: request.requestId, data: result });
    await Promise.resolve();
  }
  socket.receive({ type: "event", data: { event: "pty_output", id: "one", data: "eA==" } });
  socket.receive({ type: "event", data: { event: "pty_exit", id: "one", status: 0 } });
  expect(current()).toBe(false);
  release(); await attaching;
  await Promise.resolve(); await Promise.resolve();
  expect(context.states[context.states.length - 1].attached).toBeNull();
  expect(context.output).not.toHaveBeenCalled();
  const count = socket.sent.length;
  await expect(context.client.sendInput("echo stale\r")).rejects.toThrow();
  expect(socket.sent).toHaveLength(count);
  context.client.disconnect();
});