<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from "vue";
import { useFireworks } from "../composables/useFireworks";
import {
  spawnBurst,
  stepParticles,
  particleAlpha,
  pick,
  FIREWORK_COLORS,
  type Particle,
} from "../lib/fireworks";

// A rocket rises from the bottom, waits out its `delay`, then bursts at
// `targetY`. Rockets live in the component (canvas/rAF timing is manual-test
// scope); the spark physics come from the pure ../lib/fireworks module.
interface Rocket {
  x: number;
  y: number;
  vy: number;
  targetY: number;
  color: string;
  delay: number;
  reduce: boolean; // motion preference captured when the shell launched
}

const { state } = useFireworks();
const canvasRef = ref<HTMLCanvasElement | null>(null);

let ctx: CanvasRenderingContext2D | null = null;
let dpr = 1;
let width = 0;
let height = 0;

let rockets: Rocket[] = [];
let sparks: Particle[] = [];
let rafId = 0;
let lastTime = 0;

function prefersReducedMotion(): boolean {
  try {
    return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  } catch {
    return false;
  }
}

function resize() {
  const canvas = canvasRef.value;
  if (!canvas) return;
  dpr = window.devicePixelRatio || 1;
  width = window.innerWidth;
  height = window.innerHeight;
  canvas.width = Math.round(width * dpr);
  canvas.height = Math.round(height * dpr);
  ctx = canvas.getContext("2d");
  if (ctx) ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
}

/**
 * Queue one salvo of rockets; additive if a show is already running. Timings
 * are tuned so a full salvo bursts and clears in roughly ~2.5s: the last shell
 * launches by ~1.0s, rises ~0.5-0.8s, and its sparks fade within ~1.3s.
 */
function spawnSalvo() {
  if (!ctx) resize();
  const reduce = prefersReducedMotion();
  const shells = reduce ? 3 : 7 + Math.floor(Math.random() * 3);
  for (let i = 0; i < shells; i++) {
    const x = width * (0.15 + Math.random() * 0.7);
    const targetY = height * (0.15 + Math.random() * 0.3);
    const riseTime = 0.5 + Math.random() * 0.3;
    rockets.push({
      x,
      y: height,
      vy: -(height - targetY) / riseTime,
      targetY,
      color: pick(FIREWORK_COLORS),
      delay: reduce ? Math.random() * 0.3 : Math.random() * 1.0,
      reduce,
    });
  }
  ensureLoop();
}

function burst(r: Rocket) {
  sparks = sparks.concat(
    spawnBurst(r.x, r.targetY, {
      count: r.reduce ? 18 : 38 + Math.floor(Math.random() * 12),
      speed: 180 + Math.random() * 90,
      life: r.reduce ? 0.7 : 0.9 + Math.random() * 0.4,
      colors: [r.color, "#ffffff", pick(FIREWORK_COLORS)],
    }),
  );
}

function stepRockets(dt: number) {
  const still: Rocket[] = [];
  for (const r of rockets) {
    if (r.delay > 0) {
      r.delay -= dt;
      still.push(r);
      continue;
    }
    r.y += r.vy * dt;
    if (r.y <= r.targetY) {
      burst(r);
    } else {
      still.push(r);
    }
  }
  rockets = still;
}

function draw() {
  if (!ctx) return;
  ctx.clearRect(0, 0, width, height);
  ctx.globalCompositeOperation = "lighter";

  // Rising rockets (skip those still waiting out their delay).
  for (const r of rockets) {
    if (r.delay > 0) continue;
    ctx.globalAlpha = 1;
    ctx.fillStyle = r.color;
    ctx.beginPath();
    ctx.arc(r.x, r.y, 2, 0, Math.PI * 2);
    ctx.fill();
  }

  // Sparks fade as their life runs out.
  for (const p of sparks) {
    ctx.globalAlpha = particleAlpha(p);
    ctx.fillStyle = p.color;
    ctx.beginPath();
    ctx.arc(p.x, p.y, 2, 0, Math.PI * 2);
    ctx.fill();
  }

  ctx.globalAlpha = 1;
  ctx.globalCompositeOperation = "source-over";
}

function frame(now: number) {
  const dt = lastTime ? Math.min((now - lastTime) / 1000, 0.05) : 0;
  lastTime = now;

  stepRockets(dt);
  sparks = stepParticles(sparks, dt, { gravity: 260, drag: 0.6 });
  draw();

  if (rockets.length || sparks.length) {
    rafId = requestAnimationFrame(frame);
  } else {
    // Nothing left: clear the canvas and stop the loop (idle CPU 0).
    if (ctx) ctx.clearRect(0, 0, width, height);
    rafId = 0;
    lastTime = 0;
  }
}

function ensureLoop() {
  if (rafId) return;
  lastTime = 0;
  rafId = requestAnimationFrame(frame);
}

watch(
  () => state.token,
  () => spawnSalvo(),
);

onMounted(() => {
  resize();
  window.addEventListener("resize", resize);
});

onUnmounted(() => {
  window.removeEventListener("resize", resize);
  if (rafId) cancelAnimationFrame(rafId);
  rafId = 0;
});
</script>

<template>
  <canvas ref="canvasRef" class="fireworks-overlay" aria-hidden="true" />
</template>

<style scoped>
.fireworks-overlay {
  position: fixed;
  inset: 0;
  width: 100%;
  height: 100%;
  z-index: 1200;
  pointer-events: none;
}
</style>
