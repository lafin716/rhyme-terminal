// Pure particle model for the fireworks effect. The rendering (canvas + rAF)
// and launch timing live in FireworksOverlay.vue; this module owns only the
// spark physics so it can be unit-tested without a DOM.

export interface Particle {
  x: number;
  y: number;
  vx: number; // px per second
  vy: number; // px per second (positive = downward, screen coords)
  life: number; // seconds remaining
  maxLife: number; // seconds the spark started with (for alpha fade)
  color: string;
}

// Bright multi-color palette on the dark Shell background; includes the teal
// accent (#4ec9b0) used elsewhere. Hardcoded per the no-CSS-variables convention.
export const FIREWORK_COLORS: readonly string[] = [
  "#ff5252",
  "#ffd740",
  "#40c4ff",
  "#69f0ae",
  "#e040fb",
  "#ff6e40",
  "#4ec9b0",
  "#ffffff",
];

/** Pick a random element from a non-empty array; `rng` is injectable for tests. */
export function pick<T>(arr: readonly T[], rng: () => number = Math.random): T {
  return arr[Math.floor(rng() * arr.length) % arr.length];
}

export interface BurstOptions {
  count?: number;
  speed?: number; // base outward speed (px/s)
  life?: number; // spark lifetime in seconds
  colors?: readonly string[];
  rng?: () => number; // injectable for deterministic tests
}

/**
 * Create the spark particles of a single radial burst centered at (x, y).
 * Sparks fan out evenly around the circle with per-spark jitter and speed
 * variation drawn from `rng`.
 */
export function spawnBurst(x: number, y: number, opts: BurstOptions = {}): Particle[] {
  const count = opts.count ?? 42;
  const speed = opts.speed ?? 220;
  const life = opts.life ?? 1.2;
  const colors = opts.colors ?? FIREWORK_COLORS;
  const rng = opts.rng ?? Math.random;

  const out: Particle[] = [];
  for (let i = 0; i < count; i++) {
    const angle = (i / count) * Math.PI * 2 + (rng() - 0.5) * 0.4;
    const mag = speed * (0.35 + rng() * 0.65);
    const color = pick(colors, rng);
    out.push({
      x,
      y,
      vx: Math.cos(angle) * mag,
      vy: Math.sin(angle) * mag,
      life,
      maxLife: life,
      color,
    });
  }
  return out;
}

export interface StepOptions {
  gravity?: number; // downward acceleration (px/s^2)
  drag?: number; // per-second velocity damping (0 = none)
}

/**
 * Integrate the particle system forward by `dt` seconds, returning a fresh
 * array: positions advance by velocity, velocity gains gravity (and optional
 * drag), lifetimes shrink by `dt`, and sparks whose life reaches zero are
 * dropped. Input particles are never mutated.
 */
export function stepParticles(
  particles: readonly Particle[],
  dt: number,
  opts: StepOptions = {},
): Particle[] {
  const gravity = opts.gravity ?? 240;
  const drag = opts.drag ?? 0;
  const damp = 1 - drag * dt;

  const out: Particle[] = [];
  for (const p of particles) {
    const life = p.life - dt;
    if (life <= 0) continue;
    out.push({
      x: p.x + p.vx * dt,
      y: p.y + p.vy * dt,
      vx: p.vx * damp,
      vy: (p.vy + gravity * dt) * damp,
      life,
      maxLife: p.maxLife,
      color: p.color,
    });
  }
  return out;
}

/** Fraction of a spark's life remaining, clamped to [0, 1], for alpha fade. */
export function particleAlpha(p: Particle): number {
  if (p.maxLife <= 0) return 0;
  const a = p.life / p.maxLife;
  if (a < 0) return 0;
  if (a > 1) return 1;
  return a;
}
