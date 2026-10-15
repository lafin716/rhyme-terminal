import { ref } from "vue";

const flowOpen = ref(false);
const flowProjectId = ref<string | null>(null);

export function useFlowPage() {
  return {
    flowOpen,
    flowProjectId,
    openFlow: (projectId: string) => {
      flowProjectId.value = projectId;
      flowOpen.value = true;
    },
    closeFlow: () => {
      flowOpen.value = false;
    },
  };
}
