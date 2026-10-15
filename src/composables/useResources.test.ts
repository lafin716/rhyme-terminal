import { beforeEach, describe, expect, it, vi } from "vitest";
import { ref } from "vue";
import { makeLeaf } from "../lib/layout-types";

const mocks = vi.hoisted(() => ({ save: vi.fn(), write: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ save: mocks.save }));
vi.mock("../lib/tauri", () => ({ api: { writeFile: mocks.write } }));
vi.mock("./useI18n", () => ({ t: (s: string) => s }));
const workspace = ref({
  id: "project", settings: { defaultCwd: "C:\\projects\\demo" }, layout: makeLeaf("leaf"),
});
vi.mock("./useWorkspaces", () => ({ useWorkspaces: () => ({
  activeWorkspace: workspace, replaceLayout: vi.fn(),
}) }));
vi.mock("./useFocus", () => ({ useFocus: () => ({
  focusedLeafId: ref("leaf"), setFocusedLeaf: vi.fn(),
}) }));

import { useResources, type FileTab } from "./useResources";

describe("new page resources", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    workspace.value.layout = makeLeaf("leaf");
    workspace.value.settings.defaultCwd = "C:\\projects\\demo";
    const resources = useResources();
    for (const id of Object.keys(resources.state.tabs)) resources.forgetResource(id);
    mocks.write.mockResolvedValue(undefined);
  });

  function newPage() {
    const resources = useResources();
    resources.openNewPage();
    const id = workspace.value.layout.activeTabId!;
    return { resources, id, tab: resources.getById(id) as FileTab };
  }

  it("opens independent empty pages in the focused pane and retains drafts", () => {
    const first = newPage();
    first.resources.updateFileDraft(first.id, "draft");
    const second = newPage();
    expect(second.id).not.toBe(first.id);
    expect(second.tab.preview.text).toBe("");
    expect(first.resources.getById(first.id)).toMatchObject({ draftText: "draft", dirty: true });
    expect(workspace.value.layout.tabs).toEqual([first.id, second.id]);
  });

  it("uses the originating project for Save As and adopts the chosen path", async () => {
    const { resources, id, tab } = newPage();
    resources.updateFileDraft(id, "hello");
    workspace.value.settings.defaultCwd = "D:\\elsewhere";
    mocks.save.mockResolvedValue("C:\\projects\\demo\\note.md");
    expect(await resources.saveFile(id)).toBe(true);
    expect(mocks.save).toHaveBeenCalledWith(expect.objectContaining({ defaultPath: "C:\\projects\\demo\\Untitled.txt" }));
    expect(mocks.write).toHaveBeenCalledWith("C:\\projects\\demo\\note.md", "hello");
    expect(tab).toMatchObject({ untitled: false, dirty: false, preview: { name: "note.md", text: "hello", language: "plaintext" } });
    resources.updateFileDraft(id, "updated");
    await resources.saveFile(id);
    expect(mocks.save).toHaveBeenCalledTimes(1);
  });

  it("preserves an untitled draft after cancellation and write failure", async () => {
    const { resources, id, tab } = newPage();
    resources.updateFileDraft(id, "keep me");
    mocks.save.mockResolvedValue(null);
    expect(await resources.saveFile(id)).toBe(false);
    expect(mocks.write).not.toHaveBeenCalled();
    mocks.save.mockResolvedValue("C:\\note.txt");
    mocks.write.mockRejectedValue(new Error("denied"));
    await expect(resources.saveFile(id)).rejects.toThrow("denied");
    expect(tab).toMatchObject({ untitled: true, draftText: "keep me", dirty: true, saving: false, preview: { canonicalPath: "", text: "" } });
  });

  it("retains edits made during a write and prevents simultaneous saves", async () => {
    const { resources, id, tab } = newPage();
    resources.updateFileDraft(id, "snapshot");
    mocks.save.mockResolvedValue("C:\\note.txt");
    let finish!: () => void;
    mocks.write.mockImplementation(() => new Promise<void>((resolve) => { finish = resolve; }));
    const pending = resources.saveFile(id);
    await Promise.resolve();
    expect(await resources.saveFile(id)).toBe(false);
    resources.updateFileDraft(id, "new edits");
    finish();
    await pending;
    expect(tab).toMatchObject({ dirty: true, draftText: "new edits", preview: { text: "snapshot" } });
  });
});
