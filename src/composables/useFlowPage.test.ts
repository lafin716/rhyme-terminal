import { beforeEach, describe, expect, it } from "vitest";
import { useFlowPage } from "./useFlowPage";

const navigation = useFlowPage();
beforeEach(() => {
  navigation.closeFlow();
  navigation.flowProjectId.value = null;
});

describe("Flow page navigation", () => {
  it("starts on the terminal and opens the explicitly selected project", () => {
    expect(navigation.flowOpen.value).toBe(false);
    navigation.openFlow("background-project");
    expect(navigation.flowOpen.value).toBe(true);
    expect(navigation.flowProjectId.value).toBe("background-project");
  });

  it("keeps the project mounted on close and lets another project be selected", () => {
    navigation.openFlow("project-a");
    navigation.closeFlow();
    expect(navigation.flowOpen.value).toBe(false);
    expect(navigation.flowProjectId.value).toBe("project-a");
    navigation.openFlow("project-b");
    expect(navigation.flowProjectId.value).toBe("project-b");
    expect(useFlowPage().flowOpen.value).toBe(true);
  });
});
