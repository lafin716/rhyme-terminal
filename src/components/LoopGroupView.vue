<script setup lang="ts">
import { ref, watch, onBeforeUnmount, nextTick } from 'vue';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import TerminalView from './Terminal.vue';
import { useLoopRouting } from '../composables/useLoopRouting';
import { loopStatusLabel, type LoopGroup } from '../lib/loop-routing';
const props = defineProps<{ group: LoopGroup; active: boolean }>();
const loops = useLoopRouting();
const selected = ref('');
const host = ref<HTMLDivElement>();
const busy = ref(false);
const error = ref('');
let terminal: Terminal | undefined;
let observer: ResizeObserver | undefined;
let generation = 0;
function dispose() { terminal?.dispose(); terminal = undefined; observer?.disconnect(); }
watch(selected, async id => {
  const revision = ++generation;
  dispose(); error.value = '';
  if (!id) return;
  try {
    const data = await loops.history(props.group.id, id);
    await nextTick();
    if (revision !== generation || !host.value) return;
    terminal = new Terminal({ disableStdin: true, convertEol: false, fontSize: 13, theme: { background: '#1e1e1e' } });
    const fit = new FitAddon(); terminal.loadAddon(fit); terminal.open(host.value); fit.fit();
    terminal.write(Uint8Array.from(atob(data), c => c.charCodeAt(0)));
    observer = new ResizeObserver(() => fit.fit()); observer.observe(host.value);
  } catch (e) { if (revision === generation) error.value = String(e); }
});
onBeforeUnmount(() => { generation++; dispose(); });
async function control(op: 'pause' | 'resume' | 'next' | 'stop') {
  busy.value = true; error.value = '';
  try { await loops.control(op, props.group.id); } catch (e) { error.value = String(e); }
  finally { busy.value = false; }
}
</script>
<template>
  <section class="loop-view">
    <div class="loop-toolbar">
      <strong>↻ {{ loopStatusLabel(group.status) }}</strong>
      <select v-model="selected" aria-label="루프 실행 기록">
        <option value="">현재 실행</option>
        <option v-for="attempt in group.attempts.filter(attempt => attempt.endedAt != null && !['linked', 'queued'].includes(attempt.status))" :key="attempt.id" :value="attempt.id">{{ attempt.agent }} · {{ attempt.label }} · {{ attempt.status }} (읽기 전용)</option>
      </select>
      <button :disabled="busy || group.status === 'stopped'" @click="control(group.status === 'paused' || group.status === 'recovery' ? 'resume' : 'pause')">{{ group.status === 'paused' || group.status === 'recovery' ? '재개' : '일시정지' }}</button>
      <button :disabled="busy || group.status === 'stopped'" @click="control('next')">다음 계정</button>
      <button :disabled="busy || group.status === 'stopped'" @click="control('stop')">중지</button>
    </div>
    <p v-if="group.status === 'paused'" class="reason">일시정지 중에는 입력과 자동 전환이 잠깁니다. 계속하려면 재개를 누르세요.</p>
    <p v-if="error || group.reason" class="reason" role="status">{{ error || group.reason }}</p>
    <div v-if="selected" ref="host" class="history" aria-label="읽기 전용 실행 기록" />
    <TerminalView v-else-if="group.activeSessionId" :key="group.activeSessionId" :session-id="group.activeSessionId" :active="active" />
    <p v-else class="reason">{{ loopStatusLabel(group.status) }} · 실행 기록은 위 목록에서 확인할 수 있습니다.</p>
  </section>
</template>
<style scoped>
.loop-view :deep(.term-host){flex:1;min-height:0;height:auto}.loop-view{display:flex;flex-direction:column;height:100%;min-height:0;overflow:hidden}.loop-toolbar{display:flex;align-items:center;gap:8px;flex-wrap:wrap;padding:7px 10px;background:#252525;font-size:12px}.loop-toolbar strong{color:#4ec9b0}.loop-toolbar select{flex:1;min-width:130px}button,select{color:#ddd;background:#333;border:1px solid #555;border-radius:4px;padding:4px 7px}button:disabled{opacity:.4}.reason{padding:6px 12px;margin:0;color:#d9b879;font-size:12px}.history{flex:1;min-height:0;padding:6px}
</style>
