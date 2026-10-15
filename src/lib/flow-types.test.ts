import { describe, expect, it } from 'vitest';
import { canConnect, newFlow, newNode, removeFlowNode } from './flow-types';
import fixture from './flow-contract.fixture.json';

describe('Flow editor integrity', () => {
  it('reads the shared Rust serialization fixture without losing bindings or ports', () => {
    const flow = JSON.parse(JSON.stringify(fixture)) as ReturnType<typeof newFlow>;
    expect(flow.schema_version).toBe(1);
    expect(canConnect(flow, 'result', 'source')).toBe(false);
    expect(flow.nodes[1].input_bindings.text).toEqual({ source: 'source', path: '/stdout' });
    expect(JSON.parse(JSON.stringify(flow))).toEqual(fixture);
  });
  it('rejects loops and unknown nodes, permits independent dependencies', () => {
    const flow = newFlow();
    flow.nodes.push(newNode('approval', 'approve'));
    expect(canConnect(flow, 'result', 'command')).toBe(false);
    expect(canConnect(flow, 'command', 'command')).toBe(false);
    expect(canConnect(flow, 'missing', 'result')).toBe(false);
    expect(canConnect(flow, 'result', 'approve')).toBe(true);
  });
  it('removes dangling bindings, edges, and saved layout', () => {
    const flow = newFlow();
    removeFlowNode(flow, 'command');
    expect(flow.nodes.map(n => n.id)).toEqual(['result']);
    expect(flow.edges).toEqual([]);
    expect(flow.nodes[0].input_bindings).toEqual({});
    expect(flow.layout.command).toBeUndefined();
  });
});
