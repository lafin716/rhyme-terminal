import { it, expect } from "vitest";
import { orderNavigatorSessions, reorderSessionIds } from "./navigator";
import { makeLeaf, type SplitNode } from "./layout-types";
import { moveTabToIndex } from "../composables/useLayout";
it("reorders sidebar rows in both directions and ignores self drops", () => {
 expect(reorderSessionIds(["a","b","c"],"a","c",true)).toEqual(["b","c","a"]);
 expect(reorderSessionIds(["a","b","c"],"c","a",false)).toEqual(["c","a","b"]);
 expect(reorderSessionIds(["a","b"],"a","a",true)).toEqual(["a","b"]);
});
it("retains saved order across daemon refresh and appends new sessions", () => {
 expect(orderNavigatorSessions([{id:"a"},{id:"b"},{id:"c"}],["gone","b","a"]).map(s=>s.id)).toEqual(["b","a","c"]);
});
it("moves tabs forward and backward using insertion boundaries", () => {
 const leaf=makeLeaf("leaf",["a","b","c"],"a");
 moveTabToIndex(leaf,"a",leaf.id,3);
 expect(leaf.tabs).toEqual(["b","c","a"]);
 moveTabToIndex(leaf,"a",leaf.id,0);
 expect(leaf.tabs).toEqual(["a","b","c"]);
});
it("inserts between another group's tabs and collapses the empty source", () => {
 const source=makeLeaf("source",["a"],"a");
 const target=makeLeaf("target",["b","c"],"b");
 const root: SplitNode={kind:"split",id:"root",direction:"horizontal",sizes:[0.5,0.5],children:[source,target]};
 expect(moveTabToIndex(root,"a","target",1)).toBe(target);
 expect(target.tabs).toEqual(["b","a","c"]);
 expect(target.activeTabId).toBe("a");
});
