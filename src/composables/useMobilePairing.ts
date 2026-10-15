import { computed, onMounted, onUnmounted, ref } from "vue";
import { t } from "./useI18n";
import { emptyStatus, initialPairingIp, mobilePairingApi, PAIRING_KEY, parsePairingPreferences, serializePairingPreferences, validPort, type NetworkInterface, type PairingStatus } from "../lib/mobile-pairing";

export function useMobilePairing() {
  const status = ref(emptyStatus());
  const interfaces = ref<NetworkInterface[]>([]);
  const ip = ref("");
  const port = ref(43123);
  const busy = ref(false);
  const ready = ref(false);
  const error = ref("");
  const now = ref(Date.now());
  let disposed = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let clock: ReturnType<typeof setInterval> | undefined;
  let inFlight: Promise<void> | null = null;
  let remembered: string | null = null;
  try {
    const prefs = parsePairingPreferences(localStorage.getItem(PAIRING_KEY));
    if (prefs) { remembered = prefs.ip; ip.value = prefs.ip; port.value = prefs.port; }
  } catch { /* A denied storage read still permits explicit setup. */ }
  const selectedAvailable = computed(() => interfaces.value.some(item => item.ip === ip.value));
  const canStart = computed(() => ready.value && !busy.value && !status.value.running && selectedAvailable.value && validPort(port.value));
  const inviteSeconds = computed(() => Math.max(0, Math.ceil(((status.value.invite?.expiresAt ?? 0) - now.value) / 1000)));
  const qrSource = computed(() => status.value.invite && inviteSeconds.value > 0 ? `data:image/svg+xml;charset=utf-8,${encodeURIComponent(status.value.invite.qrSvg)}` : null);
  function apply(next: PairingStatus) {
    status.value = next;
    if (next.running && next.ip && next.port) { ip.value = next.ip; port.value = next.port; }
  }
  async function exclusive(action: () => Promise<void>): Promise<void> {
    if (disposed || inFlight) return;
    busy.value = true;
    const task = (async () => {
      try { await action(); }
      catch (cause) { if (!disposed) error.value = String(cause instanceof Error ? cause.message : cause); }
      finally { if (!disposed) busy.value = false; }
    })();
    inFlight = task;
    try { await task; } finally { if (inFlight === task) inFlight = null; }
  }
  async function refreshInterfaces() {
    await exclusive(async () => {
      const list = await mobilePairingApi.interfaces();
      if (disposed) return;
      interfaces.value = list;
      ip.value = initialPairingIp(list, ip.value || remembered);
      error.value = "";
    });
  }
  async function mutate(action: () => Promise<PairingStatus>) {
    if (inFlight || disposed) return;
    await exclusive(async () => {
      const next = await action();
      if (disposed) return;
      apply(next); error.value = "";
    });
  }
  async function start() {
    if (!canStart.value) return;
    const selection = { ip: ip.value, port: port.value };
    await mutate(() => mobilePairingApi.start(selection.ip, selection.port));
    if (!disposed && status.value.running) {
      try { localStorage.setItem(PAIRING_KEY, serializePairingPreferences(selection)); remembered = selection.ip; }
      catch { error.value = t("The server started, but the address preference could not be saved."); }
    }
  }
  function schedule() {
    if (disposed) return;
    timer = setTimeout(async () => {
      await exclusive(async () => { const next = await mobilePairingApi.status(); if (!disposed) { apply(next); ready.value = true; } });
      schedule();
    }, 1000);
  }
  onMounted(async () => {
    clock = setInterval(() => { now.value = Date.now(); }, 500);
    await exclusive(async () => {
      const [list, next] = await Promise.all([mobilePairingApi.interfaces(), mobilePairingApi.status()]);
      if (disposed) return;
      interfaces.value = list; ip.value = initialPairingIp(list, remembered); apply(next); ready.value = true;
    });
    schedule();
  });
  onUnmounted(() => { disposed = true; clearTimeout(timer); clearInterval(clock); });
  return { status, interfaces, ip, port, busy, ready, error, selectedAvailable, canStart, inviteSeconds, qrSource, refreshInterfaces, start,
    stop: () => mutate(mobilePairingApi.stop), invite: () => mutate(mobilePairingApi.invite),
    approve: (id: string, accepted: boolean) => mutate(() => mobilePairingApi.approve(id, accepted)),
    revoke: (id: string) => mutate(() => mobilePairingApi.revoke(id)),
  };
}
