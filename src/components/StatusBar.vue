<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "../composables/useI18n";
import { Icon } from "@iconify/vue";
import { useSessions, displayName } from "../composables/useSessions";
import { useWorkspaces } from "../composables/useWorkspaces";
import { useUsage, usageKey } from "../composables/useUsage";
import { CLI_AGENTS } from "../composables/useAccountProfiles";
import { sessionAgentIcon } from "../lib/session-agent-icon";
import { formatUsagePercent, formatUsageReset } from "../lib/usage-status";
import type { CliAgentKind } from "../lib/persistence";

const { focusedSession } = useSessions();
const { activeWorkspace } = useWorkspaces();
const { usage, records, targets, selected, selectToolbarProfile, summary, refresh, loading } = useUsage();
const { t, locale } = useI18n();
const now = ref(Date.now());
const time = computed(() => new Date(now.value).toLocaleTimeString(locale.value, { hour: "2-digit", minute: "2-digit" }));
const open = ref(false);
const highlighted = ref<CliAgentKind>("claude");
const dialog = ref<HTMLElement>();
let trigger: HTMLElement | null = null;
let timer: number | undefined;
let polling: number | undefined;
let sessionPolling: number | undefined;
const count = (agent: CliAgentKind) => targets.value.filter(p => p.agent === agent).length;
const tone = (percent: number | null) => percent !== null && percent >= 90 ? "danger" : percent !== null && percent >= 70 ? "warning" : "";
const stamp = (value: number) => {
  const date = new Date(value);
  return date.toDateString() === new Date(now.value).toDateString()
    ? date.toLocaleTimeString(locale.value, { hour: "2-digit", minute: "2-digit" })
    : date.toLocaleString(locale.value, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
};
async function show(agent: CliAgentKind, event: MouseEvent) {
  trigger = event.currentTarget as HTMLElement;
  highlighted.value = agent;
  open.value = true;
  void refresh();
  await nextTick();
  dialog.value?.focus();
}
function close() {
  open.value = false;
  trigger?.focus();
}
function onKey(event: KeyboardEvent) {
  if (!open.value) return;
  if (event.key === "Escape") {
    event.preventDefault(); event.stopImmediatePropagation(); close();
  } else if (event.key === "Tab") {
    const buttons = dialog.value?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)");
    if (!buttons?.length) return;
    const first = buttons[0], last = buttons[buttons.length - 1];
    if (event.shiftKey && (document.activeElement === first || document.activeElement === dialog.value)) {
      event.preventDefault(); last.focus();
    } else if (!event.shiftKey && (document.activeElement === last || document.activeElement === dialog.value)) {
      event.preventDefault(); first.focus();
    }
  }
}
watch(() => targets.value.map(p => usageKey(p.agent, p.id)).join("|"), () => void refresh());
onMounted(() => {
  void refresh();
  timer = window.setInterval(() => { now.value = Date.now(); }, 30_000);
  polling = window.setInterval(() => { if (!document.hidden) void refresh(); }, 300_000);
  sessionPolling = window.setInterval(() => { if (!document.hidden) void refresh(true); }, 10_000);
  window.addEventListener("keydown", onKey, true);
});
onUnmounted(() => {
  window.clearInterval(timer); window.clearInterval(polling); window.clearInterval(sessionPolling);
  window.removeEventListener("keydown", onKey, true);
});
</script>

<template>
  <div class="status-bar">
    <div class="left">
      <span class="badge">[rhyme-terminal]</span>
      <span v-if="activeWorkspace" class="ws">{{ activeWorkspace.name }}</span>
      <span v-if="focusedSession" class="session">/ {{ displayName(focusedSession.name) }}</span>
    </div>
    <div class="usage">
      <button v-for="agent in CLI_AGENTS" :key="agent.id" class="usage-item"
        :class="{ active: open && highlighted === agent.id }"
        :title="`${agent.label} · ${selected(agent.id).id ? selected(agent.id).label : t('System')} · ${t('View all profile usage')}`"
        aria-haspopup="dialog" :aria-expanded="open && highlighted === agent.id" aria-controls="usage-dialog"
        @click="show(agent.id, $event)">
        <Icon class="usage-ico" :icon="sessionAgentIcon(agent.id)" />
        <span>{{ agent.id === 'claude' ? 'Claude' : 'Codex' }}</span>
        <strong>{{ formatUsagePercent(usage[agent.id]) }}</strong>
        <span v-if="summary(agent.id)?.error" class="status-dot" :title="t(summary(agent.id)!.error!)">!</span>
        <span v-else-if="summary(agent.id)?.loading" class="loading-dot">···</span>
        <span v-if="count(agent.id) > 1" class="profile-count">{{ count(agent.id) }}</span>
        <Icon class="chevron" icon="lucide:chevron-up" />
      </button>
    </div>
    <div class="right">{{ time }}</div>
  </div>
  <Teleport to="body">
    <div v-if="open" class="usage-backdrop" @mousedown.self="close">
      <section id="usage-dialog" ref="dialog" class="usage-dialog" role="dialog" aria-modal="true" aria-labelledby="usage-title" tabindex="-1">
        <header class="dialog-header">
          <div><h2 id="usage-title">{{ t('Account usage') }}</h2><p>{{ t('Compare usage across all profiles') }}</p></div>
          <button class="icon-button" :title="t('Refresh usage')" :aria-label="t('Refresh usage')" :disabled="loading" @click="refresh()"><Icon :class="{ spinning: loading }" icon="lucide:refresh-cw" /></button>
          <button class="icon-button" :aria-label="t('Close')" @click="close"><Icon icon="lucide:x" /></button>
        </header>
        <div class="providers">
          <section v-for="agent in CLI_AGENTS" :key="agent.id" class="provider" :class="[agent.id, { highlighted: highlighted === agent.id }]">
            <div class="provider-heading"><Icon :icon="sessionAgentIcon(agent.id)" /><h3>{{ agent.label }}</h3><span>{{ count(agent.id) }}</span></div>
            <article v-for="profile in targets.filter(p => p.agent === agent.id)" :key="usageKey(agent.id, profile.id)" class="profile-card">
              <div class="profile-heading"><span class="profile-name" :title="profile.label">{{ profile.id ? profile.label : t('System') }}</span><button v-if="count(agent.id) > 1" type="button" class="toolbar-select" :class="{ selected: selected(agent.id).id === profile.id }" :aria-pressed="selected(agent.id).id === profile.id" :aria-label="t('Show {profile} in toolbar', { profile: profile.id ? profile.label : t('System') })" @click="selectToolbarProfile(agent.id, profile.id)"><Icon :icon="selected(agent.id).id === profile.id ? 'lucide:check' : 'lucide:pin'" />{{ t(selected(agent.id).id === profile.id ? 'Shown in toolbar' : 'Show in toolbar') }}</button><span v-else class="default-tag">{{ t('Toolbar') }}</span></div>
              <div v-for="window in records[usageKey(agent.id, profile.id)]?.windows ?? []" :key="window.label" class="window" :class="tone(window.percentUsed)">
                <div class="window-heading"><span>{{ t(window.label) }}</span><strong>{{ formatUsagePercent(window) }} <small>{{ t('used') }}</small></strong></div>
                <div class="meter" role="progressbar" :aria-label="`${profile.id ? profile.label : t('System')} · ${t(window.label)}`" :aria-valuenow="window.percentUsed ?? 0" :aria-valuemin="0" :aria-valuemax="100"><span :style="{ width: `${window.percentUsed ?? 0}%` }" /></div>
                <div class="reset">{{ window.resetsAt === null ? t('Reset time unavailable') : formatUsageReset(window, now, locale) }}</div>
              </div>
              <div v-if="records[usageKey(agent.id, profile.id)]?.error" class="error-message"><Icon icon="lucide:info" /><span>{{ t(records[usageKey(agent.id, profile.id)]!.error!) }}</span></div>
              <div v-else-if="!records[usageKey(agent.id, profile.id)]?.windows.length" class="empty-state">{{ t('Loading usage…') }}</div>
              <div v-if="records[usageKey(agent.id, profile.id)]?.updatedAt" class="updated">{{ t(records[usageKey(agent.id, profile.id)]?.error ? 'Last successful update: {time}' : profile.sessionUsage ? 'Last session update: {time}' : 'Updated {time}', { time: stamp(records[usageKey(agent.id, profile.id)]!.updatedAt!) }) }}</div>
            </article>
          </section>
        </div>
        <footer>{{ t('Toolbar shows the selected profile · Auto-refresh every 5 minutes') }}<span>{{ t('Session data is checked every 10 seconds; API requests are limited to once per minute') }}</span></footer>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.status-bar { display:flex; align-items:center; background:#4ec9b0; color:#1a1a1a; height:22px; padding:0 8px; font-size:12px; font-family:Consolas,"Cascadia Mono",monospace; user-select:none; }
.left { display:flex; gap:6px; align-items:center; min-width:0; overflow:hidden; white-space:nowrap; }
.badge,.ws { font-weight:600; } .ws,.session { overflow:hidden; text-overflow:ellipsis; }
.usage { flex:1 0 auto; display:flex; gap:3px; padding:0 12px; align-items:center; }
.usage-item { border:0; background:transparent; color:inherit; font:inherit; display:inline-flex; align-items:center; gap:6px; padding:2px 7px; height:22px; cursor:pointer; white-space:nowrap; }
.usage-item:hover,.usage-item.active { background:#00000016; }
.usage-item:focus-visible { outline:2px solid #17483f; outline-offset:-2px; }
.usage-ico { font-size:14px; } .chevron { font-size:11px; opacity:.65; }
.profile-count { font-size:10px; padding:0 4px; border-radius:3px; background:#00000012; }
.status-dot { font-weight:800; color:#743411; } .right { margin-left:auto; flex-shrink:0; }
.usage-backdrop { position:fixed; inset:0; background:#0005; z-index:1200; display:flex; align-items:flex-end; justify-content:center; padding:16px 16px 32px; }
.usage-dialog { width:760px; max-width:100%; max-height:calc(100dvh - 64px); display:flex; flex-direction:column; border:1px solid #444; border-radius:12px; background:#202124; color:#e8e8eb; box-shadow:0 20px 70px #0009; font-family:Segoe UI,sans-serif; outline:none; overflow:hidden; }
.dialog-header { display:flex; gap:8px; align-items:center; padding:20px 22px 18px; border-bottom:1px solid #ffffff0d; }
.dialog-header > div { flex:1; } h2 { font-size:17px; margin:0 0 6px; font-weight:600; } p { margin:0; color:#a0a2aa; font-size:12px; }
.icon-button { display:grid; place-items:center; width:30px; height:30px; font-size:16px; color:#b8bac3; background:transparent; border:1px solid transparent; border-radius:6px; cursor:pointer; }
.icon-button:hover { background:#ffffff0c; color:white; } .icon-button:disabled { opacity:.4; cursor:wait; } .icon-button:focus-visible { outline:2px solid #70cdb8; }
.providers { display:grid; grid-template-columns:1fr 1fr; gap:20px; padding:18px 22px 22px; overflow:auto; scrollbar-width:thin; scrollbar-color:#55575f transparent; }
.provider { min-width:0; --accent:#a3b7c9; } .provider.claude { --accent:#dcaa8c; } .provider.codex { --accent:#79cab4; }
.provider-heading { display:flex; align-items:center; gap:8px; margin:0 0 12px; color:var(--accent); font-size:19px; } h3 { font-size:13px; font-weight:600; margin:0; color:#dcdde2; } .provider-heading > span { margin-left:auto; font-size:11px; color:#93959e; }
.profile-card { border:1px solid #ffffff10; background:#28292d; border-radius:8px; padding:14px; margin-bottom:10px; }
.highlighted .profile-card { border-color:color-mix(in srgb,var(--accent) 28%,transparent); }
.profile-heading { display:flex; align-items:center; gap:8px; margin-bottom:16px; } .profile-name { font-size:13px; font-weight:600; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.toolbar-select { margin-left:auto; flex-shrink:0; display:inline-flex; align-items:center; gap:4px; padding:4px 7px; border:1px solid #ffffff20; border-radius:5px; background:transparent; color:#a8abb5; font:inherit; font-size:10px; cursor:pointer; }
.toolbar-select:hover { color:var(--accent); border-color:var(--accent); background:#ffffff06; }
.toolbar-select.selected { color:var(--accent); border-color:color-mix(in srgb,var(--accent) 35%,transparent); background:color-mix(in srgb,var(--accent) 10%,transparent); }
.toolbar-select:focus-visible { outline:2px solid var(--accent); outline-offset:2px; }
.default-tag { flex-shrink:0; font-size:10px; color:var(--accent); border:1px solid color-mix(in srgb,var(--accent) 25%,transparent); border-radius:4px; padding:2px 5px; }
.window + .window { margin-top:14px; } .window-heading { display:flex; justify-content:space-between; align-items:baseline; gap:8px; font-size:11px; color:#b6b8c1; margin-bottom:7px; } .window-heading strong { font-size:14px; color:#eee; font-variant-numeric:tabular-nums; } small { font-size:10px; font-weight:400; color:#92959f; }
.meter { height:5px; border-radius:3px; background:#ffffff0b; overflow:hidden; } .meter span { display:block; height:100%; background:var(--accent); border-radius:3px; transition:width .25s; } .warning .meter span { background:#ddb261; } .danger .meter span { background:#ef827d; }
.reset { margin-top:6px; font-size:10px; color:#9699a3; } .updated { margin-top:14px; font-size:10px; color:#858994; }
.error-message { display:flex; align-items:flex-start; gap:6px; font-size:11px; line-height:1.6; color:#d6b893; margin-top:10px; } .error-message > svg { flex-shrink:0; margin-top:3px; } .empty-state { color:#9699a3; font-size:12px; padding:8px 0; }
footer { border-top:1px solid #ffffff0d; padding:12px 22px; font-size:10px; color:#999ca7; line-height:1.7; } footer span { display:block; color:#777c88; }
.spinning { animation:spin 1s linear infinite; } @keyframes spin { to { transform:rotate(360deg); } }
@media(max-width:600px) { .providers { grid-template-columns:1fr; } .left { display:none; } .usage { padding:0; } .usage-backdrop { padding:8px 8px 28px; } }
@media(prefers-reduced-motion:reduce) { .spinning { animation:none; } .meter span { transition:none; } }
</style>
