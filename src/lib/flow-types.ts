export type NodeKind = 'agent' | 'command' | 'builtin_action' | 'condition' | 'approval' | 'bounded_repeat' | 'output';
export interface Binding { source: string; path: string }
export interface FlowNode { id: string; kind: NodeKind; definition_ref?: { id: string; revision: number }; config: Record<string, unknown>; input_bindings: Record<string, Binding>; execution_policy: { timeout_secs: number } }
export interface FlowEdge { id: string; source: string; target: string; source_port: 'success' | 'true' | 'false'; target_port: 'input' }
export interface Flow { schema_version: 1; flow_id: string; revision: number; name: string; inputs: Record<string, unknown>; outputs: Record<string, unknown>; nodes: FlowNode[]; edges: FlowEdge[]; policies: { concurrency: number; timeout_secs: number; capabilities: string[] }; layout: Record<string, { x: number; y: number }> }
export interface Worker { id: string; revision: number; name: string; provider: 'codex'; instructions: string; inputs: Record<string, unknown>; outputs: Record<string, unknown>; permissions: string[]; timeout_secs: number }
export interface FlowTask { id: string; project: string; text: string; revision: number; status: string }
export interface FlowRun { id: string; task_id: string; flow_id: string; revision: number; status: string; error?: string; created_at: number }
export interface FlowCatalog { workers: Worker[]; flows: Flow[]; tasks: FlowTask[]; runs: FlowRun[] }
export const flowId = () => crypto.randomUUID();
export function newNode(kind: NodeKind, id: string = flowId()): FlowNode {
  const config: Record<string, unknown> = kind === 'command' ? { program: 'powershell.exe', args: ['-NoProfile', '-Command', "Write-Output 'Rhyme Flow'"] } : kind === 'agent' ? { prompt: '입력 요구사항을 수행하고 결과를 설명하세요.' } : kind === 'condition' ? { binding: 'value', equals: true } : kind === 'builtin_action' ? { action: 'prepare_worktree' } : kind === 'bounded_repeat' ? { max_attempts: 3, body: [newNode('command', 'verify')], until: { node: 'verify', path: '/exit_code', equals: 0 } } : {};
  return { id, kind, config, input_bindings: {}, execution_policy: { timeout_secs: 300 } };
}
export function newFlow(): Flow {
  const first = newNode('command', 'command');
  const result = newNode('output', 'result');
  result.input_bindings = { result: { source: first.id, path: '/stdout' } };
  return { schema_version: 1, flow_id: flowId(), revision: 0, name: '새 Flow', inputs: {}, outputs: {}, nodes: [first, result], edges: [{ id: flowId(), source: first.id, target: result.id, source_port: 'success', target_port: 'input' }], policies: { concurrency: 2, timeout_secs: 1800, capabilities: ['command'] }, layout: { command: { x: 40, y: 65 }, result: { x: 300, y: 65 } } };
}
/** Remove dangling connections and data bindings together when deleting a node. */
export function removeFlowNode(flow: Flow, id: string): void {
  flow.nodes = flow.nodes.filter(node => node.id !== id);
  flow.edges = flow.edges.filter(edge => edge.source !== id && edge.target !== id);
  for (const node of flow.nodes) for (const [key, binding] of Object.entries(node.input_bindings)) if (binding.source === id) delete node.input_bindings[key];
  delete flow.layout[id];
}
export function canConnect(flow: Flow, source: string, target: string): boolean {
  if (source === target || !flow.nodes.some(n => n.id === source) || !flow.nodes.some(n => n.id === target)) return false;
  const visited = new Set<string>();
  const pending = [target];
  while (pending.length) {
    const id = pending.pop()!;
    if (id === source) return false;
    if (visited.has(id)) continue;
    visited.add(id);
    pending.push(...flow.edges.filter(edge => edge.source === id).map(edge => edge.target));
  }
  return true;
}
