import { reactive } from "vue";

// Transient trigger state for the fireworks effect. There is no persisted
// preference: the button is always on and holds no state. FireworksOverlay
// watches `token`; each launch bumps it, and mid-show launches simply add more
// shells to the running effect (additive).
interface FireworksState {
  token: number;
}

const state = reactive<FireworksState>({ token: 0 });

/** Fire a celebratory salvo of fireworks. */
function launch(): void {
  state.token++;
}

export function useFireworks() {
  return { state, launch };
}
