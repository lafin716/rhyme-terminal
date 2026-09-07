import { onMounted, onUnmounted, ref, watch, type Ref } from "vue";
import { useShellPanels } from "./useShellPanels";
import { COMPACT_MENU_WIDTH, titlebarInsets } from "../lib/titlebar-layout";

const updates = new Set<() => void>();
let observer: ResizeObserver | undefined;
function updateAll() { updates.forEach(update => update()); }

export function useTitlebarInsets(row: Ref<HTMLElement | null>) {
  const { panels } = useShellPanels();
  let observedRow: HTMLElement | null = null;
  const style = ref({ marginLeft: "0px", width: "100%" });
  function update() {
    if (!row.value) return;
    const { left, right } = titlebarInsets(
      row.value.getBoundingClientRect(), window.innerWidth,
      panels.left.open ? panels.left.width : COMPACT_MENU_WIDTH,
    );
    style.value = { marginLeft: `${left}px`, width: `calc(100% - ${left + right}px)` };
  }
  watch(() => [panels.left.open, panels.left.width, panels.right.open, panels.right.width], updateAll, { flush: "post" });
  onMounted(() => {
    observer ??= new ResizeObserver(updateAll);
    updates.add(update);
    observedRow = row.value;
    if (observedRow) observer.observe(observedRow);
    window.addEventListener("resize", update);
    updateAll();
  });
  onUnmounted(() => {
    if (observedRow) observer?.unobserve(observedRow);
    updates.delete(update);
    window.removeEventListener("resize", update);
    if (!updates.size) { observer?.disconnect(); observer = undefined; }
  });
  return style;
}
