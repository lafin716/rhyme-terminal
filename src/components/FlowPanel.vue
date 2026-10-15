<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { profilesForAgent } from '../composables/useAccountProfiles';
import { canConnect, flowId, newFlow, newNode, removeFlowNode, type Flow, type FlowCatalog, type FlowEdge, type FlowNode, type FlowRun, type FlowTask, type NodeKind, type Worker } from '../lib/flow-types';

const props = defineProps<{ project: string; projectId: string; active: boolean }>();
const tab = defineModel<'designer' | 'workers' | 'tasks' | 'runs'>('tab', { default: 'designer' });
const catalog = ref<FlowCatalog>({ workers: [], flows: [], tasks: [], runs: [] });
const flow = ref<Flow>(newFlow());
const selected = ref(flow.value.nodes[0].id);
const node = computed(() => flow.value.nodes.find(n => n.id === selected.value));
const nodeJson = ref('');
const flowJson = ref('');
const workerJson = ref('');
const workerSelected = ref('');
const prompt = ref('');
const preview = ref('');
const previewKind = ref<'flow' | 'worker'>('flow');
const error = ref('');
const notice = ref('');
const busy = ref(false);
const profileId = ref('');
const profiles = computed(() => profilesForAgent('codex'));
const kind = ref<NodeKind>('command');
const kinds: NodeKind[] = ['agent', 'command', 'builtin_action', 'condition', 'approval', 'bounded_repeat', 'output'];
const from = ref('');
const to = ref('');
const port = ref<FlowEdge['source_port']>('success');
const chat = ref('');
const messages = ref<{ message_id: string; text: string; classification: string }[]>([]);
const intakePlan = ref('');
watch(chat, () => { intakePlan.value = ''; });
function analyzeMessage() { void action(async () => { intakePlan.value = pretty(await request('intake_preview', { project: props.project, text: chat.value, profile_id: profileId.value || undefined })); notice.value = '의도와 기존 Task·Flow 추천을 검토한 뒤 저장하세요.'; }); }
const taskId = ref('');
const taskEdit = ref('');
const decisions = ref<unknown>(null);
const runId = ref('');
const detail = ref<{ run: FlowRun; attempts: Record<string, unknown>[]; events: unknown[] } | null>(null);
const capabilityOptions = ['command', 'agent_read', 'workspace_write', 'git_write', 'external_publish'];
const capabilities = ref<string[]>([]);
const runInputs = ref('{}');
const importInput = ref<HTMLInputElement>();
const projectKey = (path: string) => path.replace(/^\\\\\?\\/, '').replace(/\//g, '\\').replace(/\\+$/, '').toLowerCase();
const tasks = computed(() => catalog.value.tasks.filter(t => projectKey(t.project) === projectKey(props.project)));
const runs = computed(() => catalog.value.runs.filter(r => tasks.value.some(t => t.id === r.task_id)));
let timer: ReturnType<typeof setInterval> | undefined;
let polling = false;
let disposed = false;
let drag: { id: string; x: number; y: number; left: number; top: number } | undefined;
const pretty = (value: unknown) => JSON.stringify(value, null, 2);
const request = <T,>(op: string, data: Record<string, unknown> = {}) => invoke<T>('flow_request', { request: { op, ...data } });
async function action(fn: () => Promise<void>) {
  if (busy.value) return;
  busy.value = true; error.value = ''; notice.value = '';
  try { await fn(); } catch (e) { error.value = String(e); } finally { busy.value = false; }
}
async function refresh() {
  if (polling || disposed || !props.active) return;
  polling = true;
  try {
    const next = await request<FlowCatalog>('catalog');
    if (disposed) return;
    catalog.value = next;
    if (props.project && tab.value === 'tasks') messages.value = await request('messages', { project: props.project });
    if (runId.value) {
      const id = runId.value;
      const nextDetail = await request<typeof detail.value>('detail', { run_id: id });
      if (!disposed && id === runId.value) detail.value = nextDetail;
    }
  } catch (e) { if (!disposed) error.value = String(e); } finally { polling = false; }
}
function syncNode() { nodeJson.value = node.value ? pretty(node.value) : ''; }
watch(selected, syncNode);
watch(() => props.active, active => { if (active) void refresh(); });
watch(taskId, () => { taskEdit.value = tasks.value.find(t => t.id === taskId.value)?.text ?? ''; });
watch(() => props.project, () => { taskId.value = ''; runId.value = ''; detail.value = null; });
function useFlow(value: Flow) { flow.value = JSON.parse(JSON.stringify(value)) as Flow; selected.value = value.nodes[0]?.id ?? ''; flowJson.value = pretty(value); syncNode(); preview.value = ''; }
function freshFlow() { useFlow(newFlow()); }
function developmentPackage() { void action(async () => { const result = await request<{ flow: Flow; configuration_required: string[] }>('development_package'); useFlow(result.flow); notice.value = result.configuration_required.join('\n'); await refresh(); }); }
function freshWorker() {
  workerSelected.value = '';
  workerJson.value = pretty({ id: flowId(), revision: 0, name: '개발자 워커', provider: 'codex', instructions: '기존 구조를 먼저 조사하고 요구사항에 필요한 최소한의 코드를 수정합니다. 검증 결과를 보고합니다.', inputs: {}, outputs: {}, permissions: ['agent_read'], timeout_secs: 600 } satisfies Worker);
}
function loadWorker(worker: Worker) { workerSelected.value = worker.id; workerJson.value = pretty(worker); }
function addNode() { const value = newNode(kind.value); flow.value.nodes.push(value); flow.value.layout[value.id] = { x: 60 + flow.value.nodes.length * 25, y: 180 }; selected.value = value.id; syncNode(); }
function removeNode() { removeFlowNode(flow.value, selected.value); selected.value = flow.value.nodes[0]?.id ?? ''; syncNode(); }
function connect() {
  if (!canConnect(flow.value, from.value, to.value)) { error.value = '존재하는 서로 다른 노드를 선택하세요. 순환 연결은 허용되지 않습니다.'; return; }
  if (flow.value.edges.some(e => e.source === from.value && e.target === to.value && e.source_port === port.value)) return;
  flow.value.edges.push({ id: flowId(), source: from.value, target: to.value, source_port: port.value, target_port: 'input' });
}
function position(id: string) { return flow.value.layout[id] ?? { x: 30, y: 30 }; }
function edgePath(edge: FlowEdge) { const a = position(edge.source); const b = position(edge.target); return `M ${a.x + 174} ${a.y + 35} C ${a.x + 230} ${a.y + 35}, ${b.x - 60} ${b.y + 35}, ${b.x} ${b.y + 35}`; }
function startDrag(event: PointerEvent, id: string) {
  selected.value = id;
  const p = position(id); drag = { id, x: event.clientX, y: event.clientY, left: p.x, top: p.y };
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}
function moveDrag(event: PointerEvent) { if (drag) flow.value.layout[drag.id] = { x: Math.max(0, drag.left + event.clientX - drag.x), y: Math.max(0, drag.top + event.clientY - drag.y) }; }
function applyNode() {
  try {
    const value = JSON.parse(nodeJson.value) as FlowNode;
    if (value.id !== selected.value) throw new Error('노드 ID는 변경할 수 없습니다. 새 노드를 추가하세요.');
    if (!kinds.includes(value.kind) || !value.config || !value.input_bindings || !value.execution_policy) throw new Error('노드 종류, config, input_bindings, execution_policy가 필요합니다.');
    flow.value.nodes.splice(flow.value.nodes.findIndex(n => n.id === selected.value), 1, value); notice.value = '노드 초안에 적용했습니다. 저장 전 서버 검증을 수행합니다.';
  } catch (e) { error.value = String(e); }
}
function saveFlow() { void action(async () => { await request('validate', { flow: flow.value }); const saved = await request<Flow>('save_flow', { flow: flow.value }); useFlow(saved); notice.value = `Flow v${saved.revision} 확정 완료`; await refresh(); }); }
function validate() { void action(async () => { await request('validate', { flow: flow.value }); notice.value = 'Flow 검증 통과'; }); }
function saveWorker() { void action(async () => { const saved = await request<Worker>('save_worker', { worker: JSON.parse(workerJson.value) }); loadWorker(saved); notice.value = `워커 v${saved.revision} 확정 완료`; await refresh(); }); }
function deleteWorker() { void action(async () => { await request('delete_worker', { id: workerSelected.value }); freshWorker(); notice.value = '워커를 카탈로그에서 삭제했습니다.'; await refresh(); }); }
function design(target: 'flow' | 'worker') { void action(async () => {
  if (!prompt.value.trim()) throw new Error('만들거나 수정할 내용을 입력하세요.');
  const current = target === 'flow' ? flow.value : JSON.parse(workerJson.value);
  const draft = await request<unknown>('design', { kind: target, prompt: prompt.value, current, project: props.project, profile_id: profileId.value || undefined });
  previewKind.value = target; preview.value = pretty(draft); notice.value = 'AI 변경안을 검토한 뒤 초안에 적용하세요. 아직 확정되거나 실행되지 않았습니다.';
}); }
function applyPreview() { void action(async () => { const value = JSON.parse(preview.value); if (previewKind.value === 'flow') { await request('validate', { flow: value }); useFlow(value); } else { workerJson.value = pretty(value); preview.value = ''; } notice.value = '변경안을 초안에 적용했습니다. 버전 확정 버튼으로 저장하세요.'; }); }
function exportFlow() {
  const url = URL.createObjectURL(new Blob([pretty(flow.value)], { type: 'application/json' }));
  const link = document.createElement('a'); link.href = url; link.download = `${flow.value.flow_id}.json`; link.click(); setTimeout(() => URL.revokeObjectURL(url), 1000);
}
function importFlow(event: Event) { const input = event.target as HTMLInputElement; const file = input.files?.[0]; if (file) void action(async () => { const value = JSON.parse(await file.text()); await request('validate', { flow: value }); useFlow(value); notice.value = 'JSON을 초안으로 불러왔습니다.'; }); input.value = ''; }
function applyFlowJson() { void action(async () => { const value = JSON.parse(flowJson.value); await request('validate', { flow: value }); useFlow(value); notice.value = '전체 JSON을 초안에 적용했습니다.'; }); }
let pendingMessage: { text: string; project: string; task: string; id: string } | undefined;
function sendMessage() { void action(async () => {
  if (!props.project || !chat.value.trim()) throw new Error('프로젝트 경로와 메시지가 필요합니다.');
  if (!pendingMessage || pendingMessage.text !== chat.value || pendingMessage.project !== props.project || pendingMessage.task !== taskId.value) pendingMessage = { text: chat.value, project: props.project, task: taskId.value, id: flowId() };
  const result = await request<{ decisions: { flow_id?: string }[]; tasks: FlowTask[] }>('intake', { project: props.project, text: pendingMessage.text, client_id: pendingMessage.id, task_id: taskId.value || undefined, plan: intakePlan.value ? JSON.parse(intakePlan.value) : undefined });
  decisions.value = result.decisions; const recommended = catalog.value.flows.find(f => f.flow_id === result.decisions[0]?.flow_id); if (recommended) useFlow(recommended); if (result.tasks[0]) taskId.value = result.tasks[0].id; chat.value = ''; pendingMessage = undefined; await refresh(); notice.value = '요구사항을 분류하여 저장했습니다. 아래에서 Flow를 선택하고 실행하세요.';
}); }
function updateTask() { void action(async () => { await request('update_task', { task_id: taskId.value, text: taskEdit.value, expected_revision: tasks.value.find(t => t.id === taskId.value)?.revision }); await refresh(); notice.value = '새 요구사항 버전을 저장했습니다.'; }); }
function startRun() { void action(async () => {
  if (!props.project || !taskId.value || flow.value.revision < 1) throw new Error('프로젝트, Task, 확정된 Flow 버전이 필요합니다.');
  const run = await request<FlowRun>('run', { flow_id: flow.value.flow_id, revision: flow.value.revision, task_id: taskId.value, project: props.project, capabilities: capabilities.value, inputs: JSON.parse(runInputs.value), profile_id: profileId.value || undefined });
  runId.value = run.id; tab.value = 'runs'; await refresh();
}); }
function control(actionName: 'cancel' | 'approve' | 'reject', nodeId?: string) { void action(async () => { await request('control', { run_id: runId.value, action: actionName, node_id: nodeId }); await refresh(); }); }
function reconcileDelivery() { void action(async () => { await request('reconcile_delivery', { run_id: runId.value }); await refresh(); notice.value = '원격 PR 결과를 대조했습니다.'; }); }
function retry() { void action(async () => { const run = await request<FlowRun>('retry', { run_id: runId.value, capabilities: capabilities.value }); runId.value = run.id; await refresh(); }); }
function selectRun(id: string) { runId.value = id; detail.value = null; void refresh(); }
function approvalNode(attempt: Record<string, unknown>) { return String(attempt.node_id ?? ''); }
onMounted(() => { syncNode(); freshWorker(); flowJson.value = pretty(flow.value); void refresh(); timer = setInterval(() => void refresh(), 1000); });
onUnmounted(() => { disposed = true; if (timer) clearInterval(timer); });
</script>

<template>
  <section class="flow-panel" aria-label="Rhyme Flow">
    <header class="flow-header"><div><strong>Rhyme Flow</strong><small>로컬 워커 · 업무 자동화</small></div><span class="project" :title="project">{{ project || '프로젝트 폴더 설정 필요' }}</span><select v-model="profileId" aria-label="Codex 프로필"><option value="">기본 Codex 프로필</option><option v-for="p in profiles" :key="p.id" :value="p.id">{{ p.label }}</option></select></header>
    <div v-if="error" class="banner error" role="alert">{{ error }}<button @click="error = ''">닫기</button></div>
    <div v-if="notice" class="banner" role="status">{{ notice }}</div>
    <div v-if="!project" class="banner">프로젝트의 루트 폴더를 설정하면 관리형 실행과 프로젝트 채팅을 사용할 수 있습니다.</div>
    <div class="body">
      <template v-if="tab === 'designer'">
        <div class="toolbar"><select aria-label="저장된 Flow 버전" @change="useFlow(catalog.flows[Number(($event.target as HTMLSelectElement).value)])"><option value="" selected disabled>저장된 Flow 불러오기</option><option v-for="(f, index) in catalog.flows" :key="`${f.flow_id}:${f.revision}`" :value="index">{{ f.name }} · v{{ f.revision }}</option></select><button @click="freshFlow">새 Flow</button><button :disabled="busy" @click="developmentPackage">개발 패키지</button><button @click="exportFlow">JSON 내보내기</button><button @click="importInput?.click()">JSON 가져오기</button><input ref="importInput" type="file" accept="application/json,.json" hidden @change="importFlow"><button :disabled="busy" @click="validate">검증</button><button class="primary" :disabled="busy" @click="saveFlow">새 버전 확정</button></div>
        <div class="toolbar"><input v-model="flow.name" aria-label="Flow 이름"><span>초안 · 기준 v{{ flow.revision }}</span><select v-model="kind" aria-label="추가할 노드 종류"><option v-for="k in kinds" :key="k">{{ k }}</option></select><button @click="addNode">노드 추가</button></div>
        <div class="designer">
          <div class="canvas-scroll"><div class="canvas" :style="{ width: Math.max(850, ...Object.values(flow.layout).map(p => p.x + 220)) + 'px', height: Math.max(450, ...Object.values(flow.layout).map(p => p.y + 120)) + 'px' }">
            <svg class="connections" width="100%" height="100%"><defs><marker id="flow-arrow" markerWidth="8" markerHeight="8" refX="7" refY="3" orient="auto"><path d="M0,0 L0,6 L8,3 z" fill="#6ea6bd" /></marker></defs><g v-for="edge in flow.edges" :key="edge.id"><path :d="edgePath(edge)" fill="none" stroke="#6ea6bd" stroke-width="2" marker-end="url(#flow-arrow)" /><text :x="(position(edge.source).x + position(edge.target).x + 174) / 2" :y="(position(edge.source).y + position(edge.target).y) / 2 + 24">{{ edge.source_port }}</text></g></svg>
            <button v-for="n in flow.nodes" :key="n.id" class="node" :class="{ selected: selected === n.id }" :style="{ left: position(n.id).x + 'px', top: position(n.id).y + 'px' }" @pointerdown="startDrag($event, n.id)" @pointermove="moveDrag" @pointerup="drag = undefined" @pointercancel="drag = undefined" @click="selected = n.id"><span>{{ n.kind }}</span><strong>{{ n.id }}</strong><small v-if="n.definition_ref">{{ n.definition_ref.id }} v{{ n.definition_ref.revision }}</small></button>
          </div></div>
          <aside><template v-if="node"><div class="toolbar"><strong>노드 편집</strong><button @click="removeNode">삭제</button></div><p>config와 input_bindings를 수정합니다. 참조 경로는 JSON Pointer입니다.</p><textarea v-model="nodeJson" class="code inspector" aria-label="노드 JSON" spellcheck="false" /><div v-if="node.kind === 'agent'" class="hint">definition_ref에 저장된 워커의 id와 revision을 지정하세요.</div><button @click="applyNode">노드에 적용</button></template></aside>
        </div>
        <div class="toolbar"><strong>연결</strong><select v-model="from" aria-label="시작 노드"><option value="">시작</option><option v-for="n in flow.nodes" :key="n.id">{{ n.id }}</option></select><select v-model="port" aria-label="조건 포트"><option>success</option><option>true</option><option>false</option></select><span>→</span><select v-model="to" aria-label="도착 노드"><option value="">도착</option><option v-for="n in flow.nodes" :key="n.id">{{ n.id }}</option></select><button @click="connect">연결 추가</button></div>
        <div class="edges"><button v-for="edge in flow.edges" :key="edge.id" :title="'연결 삭제: ' + edge.id" @click="flow.edges = flow.edges.filter(e => e.id !== edge.id)">{{ edge.source }} → {{ edge.target }} ({{ edge.source_port }}) ×</button></div>
        <details><summary @click="flowJson = pretty(flow)">전체 Flow JSON · 입력/출력, 정책, 반복 본문 편집</summary><textarea v-model="flowJson" class="code large" aria-label="전체 Flow JSON" spellcheck="false" /><button :disabled="busy" @click="applyFlowJson">검증 후 초안에 적용</button></details>
      </template>
      <template v-if="tab === 'workers'">
        <div class="toolbar"><button class="primary" @click="freshWorker">새 워커</button><button :disabled="busy" @click="saveWorker">새 버전 확정</button><button :disabled="!workerSelected || busy" @click="deleteWorker">선택 워커 삭제</button></div>
        <div class="worker-layout"><aside><button v-for="w in catalog.workers" :key="`${w.id}:${w.revision}`" class="list-item" :class="{ active: workerSelected === w.id }" @click="loadWorker(w)"><strong>{{ w.name }}</strong><small>{{ w.provider }} · v{{ w.revision }}</small></button><p v-if="!catalog.workers.length">아직 저장한 워커가 없습니다.</p></aside><div><p>역할 지침, inputs/outputs 계약, permissions와 실행 제한을 편집하세요. 워커 생성은 실행을 시작하지 않습니다.</p><textarea v-model="workerJson" class="code large" aria-label="워커 정의 JSON" spellcheck="false" /></div></div>
      </template>
      <section v-if="tab === 'designer' || tab === 'workers'" class="ai-editor"><strong>자연어로 {{ tab === 'workers' ? '워커' : 'Flow' }} 만들기 · 수정하기</strong><textarea v-model="prompt" aria-label="자연어 설계 요청" placeholder="예: 기존 워커로 개발 → 검증 → 리뷰를 수행하고, 실패하면 최대 3번 반복하는 Flow를 만들어줘." /><button :disabled="busy || !prompt.trim()" @click="design(tab === 'workers' ? 'worker' : 'flow')">{{ busy ? '처리 중…' : 'Codex 변경안 생성' }}</button><small>Codex CLI와 선택 프로필의 인증이 필요합니다. 연결 실패는 위 오류에 표시됩니다.</small><div v-if="preview" class="preview"><strong>변경 미리보기 · {{ previewKind }}</strong><div class="diff"><div><small>현재 정의</small><pre>{{ previewKind === 'flow' ? pretty(flow) : workerJson }}</pre></div><div><small>제안된 정의 · 편집 가능</small><textarea v-model="preview" class="code large" aria-label="AI 변경안" /></div></div><button class="primary" :disabled="busy" @click="applyPreview">검토한 변경안을 초안에 적용</button><button @click="preview = ''">버리기</button></div></section>
      <template v-if="tab === 'tasks'">
        <h3>프로젝트 채팅</h3><p>새 업무는 Task로 분리하고 기존 업무의 중복과 추가 요구사항을 대조합니다. 선택한 Task에 후속 요구사항을 연결할 수 있습니다.</p>
        <select v-model="taskId" aria-label="후속 요구사항 대상 Task"><option value="">자동 분류 · 새 업무 / 기존 업무</option><option v-for="t in tasks" :key="t.id" :value="t.id">{{ t.text.slice(0, 90) }} · v{{ t.revision }}</option></select>
        <textarea v-model="chat" class="chat" aria-label="프로젝트 메시지" placeholder="이번 프로젝트에서 할 일을 설명하세요. 여러 요구사항을 함께 입력할 수 있습니다." /><button class="primary" :disabled="busy || !project || !chat.trim()" @click="sendMessage">메시지 저장 · 업무 분류</button><button :disabled="busy || !project || !chat.trim()" @click="analyzeMessage">AI 의도 분리 · 기존 업무 검색</button><textarea v-if="intakePlan" v-model="intakePlan" class="code large" aria-label="업무 분류 검토" /><p class="hint">AI 분석 없이 저장하면 줄 단위로 분리하고 동일 문장 또는 선택한 Task를 재사용합니다. 추가 요구사항은 다음 실행에 적용됩니다.</p><pre v-if="decisions">{{ pretty(decisions) }}</pre>
        <details><summary>최근 프로젝트 메시지</summary><article v-for="message in messages" :key="message.message_id" class="attempt"><p>{{ message.text }}</p><small>{{ message.classification }}</small></article></details><div class="task-list"><button v-for="t in tasks" :key="t.id" class="list-item" :class="{ active: taskId === t.id }" @click="taskId = t.id"><strong>{{ t.text }}</strong><small>{{ t.status }} · 요구사항 v{{ t.revision }} · {{ t.id }}</small></button></div>
        <div v-if="taskId"><h3>요구사항 수정</h3><textarea v-model="taskEdit" aria-label="요구사항 새 버전" /><button :disabled="busy || !taskEdit.trim()" @click="updateTask">새 요구사항 버전 저장</button></div>
        <section class="run-config"><h3>관리형 실행</h3><select aria-label="실행할 Flow" @change="useFlow(catalog.flows[Number(($event.target as HTMLSelectElement).value)])"><option value="" selected disabled>확정된 Flow 선택</option><option v-for="(f, index) in catalog.flows" :key="`${f.flow_id}:${f.revision}`" :value="index">{{ f.name }} · v{{ f.revision }}</option></select><p>{{ flow.name }} · v{{ flow.revision }} · {{ taskId ? '선택한 Task' : 'Task를 선택하세요' }}</p><label>추가 실행 입력 JSON<textarea v-model="runInputs" class="code" aria-label="실행 입력 JSON" /></label><p>이 실행에 허용할 권한</p><label v-for="cap in capabilityOptions" :key="cap" class="capability"><input v-model="capabilities" type="checkbox" :value="cap">{{ cap }}</label><p class="hint">command는 현재 사용자 권한으로 프로그램을 실행하며 파일·네트워크에 접근할 수 있습니다. 게시 액션은 external_publish 선택과 별도 승인을 모두 요구합니다.</p><button class="primary" :disabled="busy || !project || !taskId || flow.revision < 1" @click="startRun">확정 버전 실행</button></section>
      </template>
      <template v-if="tab === 'runs'">
        <div class="banner">화면을 이동해도 실행은 유지됩니다. 앱 프로세스 종료 후 계속 실행되는 서비스는 아닙니다. 재시작 시 기록을 대조하고 중단된 실행을 확인한 뒤 재시도하세요.</div>
        <div class="worker-layout"><aside><button v-for="r in runs" :key="r.id" class="list-item" :class="{ active: runId === r.id }" @click="selectRun(r.id)"><strong>{{ r.status }}</strong><small>{{ r.flow_id }} · v{{ r.revision }}</small><small>{{ r.id }}</small></button><p v-if="!runs.length">이 프로젝트의 실행 기록이 없습니다.</p></aside><div v-if="detail"><div class="toolbar"><strong>{{ detail.run.status }}</strong><button :disabled="busy" @click="control('cancel')">취소</button><button :disabled="busy" @click="retry">선택 권한으로 새 실행</button><button :disabled="busy" @click="reconcileDelivery">원격 PR 결과 대조</button></div><p v-if="detail.run.error" class="error">{{ detail.run.error }}</p><h3>실행 스냅샷</h3><pre>{{ pretty(detail.run) }}</pre><h3>단계별 입출력 · 로그 · 증거</h3><article v-for="(attempt, index) in detail.attempts" :key="String(attempt.id ?? index)" class="attempt"><strong>{{ attempt.node_id }} · {{ attempt.status }}</strong><div v-if="String(attempt.status).includes('approval') || String(attempt.status) === 'waiting'"><button class="primary" :disabled="busy" @click="control('approve', approvalNode(attempt))">이 단계 승인</button><button :disabled="busy" @click="control('reject', approvalNode(attempt))">거절</button></div><pre>{{ pretty(attempt) }}</pre></article><h3>감사 이벤트</h3><pre>{{ pretty(detail.events) }}</pre></div><p v-else>실행을 선택하면 단계별 기록을 볼 수 있습니다.</p></div>
      </template>
    </div>
  </section>
</template>

<style scoped>
.flow-panel{height:100%;min-height:0;display:flex;flex-direction:column;background:var(--bg-primary,#15181e);color:var(--text-primary,#e1e5ed);font-size:12px;overflow:hidden}.flow-header{display:flex;align-items:center;gap:20px;padding:18px 22px;border-bottom:1px solid #ffffff12}.flow-header strong{font-size:20px;letter-spacing:-.5px}.flow-header small{display:block;color:#91a0b4;margin-top:3px}.project{flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;color:#98a5b7}nav{display:flex;gap:5px;padding:8px 20px;border-bottom:1px solid #ffffff12}button,select,input,textarea{font:inherit;color:inherit;background:#202630;border:1px solid #ffffff19;border-radius:5px}button{padding:7px 11px;cursor:pointer}button:hover{background:#2c3543}button:disabled{opacity:.45;cursor:default}button.primary{background:#256d68;border-color:#398e83}button.active,nav button.active{background:#2e3e4b;border-color:#638e9e}input,select{padding:7px;max-width:100%}textarea{width:100%;min-height:75px;padding:10px;box-sizing:border-box;resize:vertical;line-height:1.6}button:focus-visible,input:focus-visible,textarea:focus-visible,select:focus-visible{outline:2px solid #7bc5c0;outline-offset:2px}.body{padding:16px 22px;overflow:auto;flex:1;min-height:0}.toolbar{display:flex;align-items:center;gap:8px;flex-wrap:wrap;margin-bottom:12px}.toolbar span{color:#91a0b4}.banner{padding:10px 18px;background:#20322f;color:#b5dace;white-space:pre-wrap;overflow-wrap:anywhere}.banner button{float:right}.error{color:#ffc2b9;background:#482b2d}.designer{display:grid;grid-template-columns:minmax(250px,1fr) 320px;gap:12px}.canvas-scroll{min-height:450px;max-height:560px;overflow:auto;background:#11161d;border:1px solid #ffffff14;border-radius:7px}.canvas{position:relative;background-image:radial-gradient(#ffffff19 1px,transparent 1px);background-size:20px 20px}.connections{position:absolute;pointer-events:none}.connections text{fill:#93aabd;font-size:10px}.node{position:absolute;width:174px;min-height:70px;text-align:left;touch-action:none;user-select:none;background:#222b36;border:1px solid #52677a;box-shadow:0 4px 15px #0004;overflow:hidden}.node.selected{border:2px solid #81cbbd}.node span{font-size:10px;color:#87c8bb;text-transform:uppercase}.node strong{display:block;margin-top:5px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.node small{display:block;color:#94a5b6}.code,pre{font-family:Consolas,monospace;font-size:11px;line-height:1.55;tab-size:2}.inspector{min-height:335px}.large{min-height:330px}.hint,p{color:#99a7b7;line-height:1.7}.edges{display:flex;gap:5px;flex-wrap:wrap;margin:10px 0}.edges button{font-size:10px}details{margin:18px 0}summary{cursor:pointer;color:#a9b9cc;padding:8px 0}.ai-editor,.run-config{margin-top:22px;padding:16px;border:1px solid #ffffff17;border-radius:7px;background:#1b2129}.ai-editor textarea{margin:10px 0}.ai-editor>small{display:block;color:#8c9bab;margin-top:9px}.diff{display:grid;grid-template-columns:1fr 1fr;gap:10px;min-width:0}.diff>div{min-width:0}.diff pre{height:330px;overflow:auto}.worker-layout{display:grid;grid-template-columns:240px minmax(0,1fr);gap:18px}.list-item{display:block;width:100%;text-align:left;margin-bottom:7px;overflow-wrap:anywhere}.list-item strong{font-weight:500;white-space:pre-wrap}.list-item small{display:block;color:#96a7ba;margin-top:6px;font-size:10px}.task-list{margin-top:20px}.chat{display:block;margin:12px 0;min-height:110px}.capability{display:inline-flex;gap:5px;align-items:center;margin:6px 12px 6px 0}.attempt{padding:12px;margin:12px 0;border:1px solid #ffffff17;border-radius:6px}pre{white-space:pre-wrap;overflow-wrap:anywhere;background:#10151c;border-radius:5px;padding:12px;max-height:450px;overflow:auto}h3{font-size:13px;margin:20px 0 10px}.preview{margin-top:18px}@media(max-width:900px){.designer,.worker-layout{grid-template-columns:1fr}.inspector{min-height:220px}.flow-header{flex-wrap:wrap}.diff{grid-template-columns:1fr}}
</style>
