import { describe, it, expect } from "vitest";
import { spawnBurst, stepParticles, particleAlpha, pick, FIREWORK_COLORS, type Particle } from "./fireworks";

const half = () => 0.5;

function spark(over: Partial<Particle> = {}): Particle {
  return { x: 0, y: 0, vx: 10, vy: -20, life: 1, maxLife: 1, color: "#fff", ...over };
}

describe("spawnBurst", () => {
  it("creates the requested number of spark particles", () => {
    const sparks = spawnBurst(100, 50, { count: 24, rng: half });
    expect(sparks).toHaveLength(24);
  });

  it("places every spark at the burst origin", () => {
    const sparks = spawnBurst(100, 50, { count: 8, rng: half });
    for (const p of sparks) {
      expect(p.x).toBe(100);
      expect(p.y).toBe(50);
    }
  });

  it("gives every spark a full positive lifetime", () => {
    const sparks = spawnBurst(0, 0, { count: 8, life: 1.2, rng: half });
    for (const p of sparks) {
      expect(p.life).toBe(1.2);
      expect(p.maxLife).toBe(1.2);
      expect(p.life).toBeGreaterThan(0);
    }
  });

  it("gives every spark a nonzero outward velocity", () => {
    const sparks = spawnBurst(0, 0, { count: 16, rng: half });
    for (const p of sparks) {
      const speed = Math.hypot(p.vx, p.vy);
      expect(speed).toBeGreaterThan(0);
    }
  });

  it("spreads sparks in different directions (radial burst)", () => {
    const sparks = spawnBurst(0, 0, { count: 16, rng: half });
    const angles = new Set(sparks.map((p) => Math.round(Math.atan2(p.vy, p.vx) * 100)));
    expect(angles.size).toBeGreaterThan(1);
  });

  it("colors sparks from the given palette", () => {
    const sparks = spawnBurst(0, 0, { count: 8, colors: ["#abc"], rng: half });
    for (const p of sparks) expect(p.color).toBe("#abc");
  });

  it("defaults to the fireworks palette", () => {
    const sparks = spawnBurst(0, 0, { count: 8, rng: half });
    for (const p of sparks) expect(FIREWORK_COLORS).toContain(p.color);
  });
});

describe("stepParticles", () => {
  it("advances each spark's position by its velocity over dt", () => {
    const [p] = stepParticles([spark({ x: 100, y: 50, vx: 10, vy: -20 })], 0.5, { gravity: 0 });
    expect(p.x).toBeCloseTo(105); // 100 + 10 * 0.5
    expect(p.y).toBeCloseTo(40); // 50 + -20 * 0.5
  });

  it("accelerates sparks downward under gravity", () => {
    const [p] = stepParticles([spark({ vy: -20 })], 0.5, { gravity: 100 });
    expect(p.vy).toBeCloseTo(30); // -20 + 100 * 0.5
  });

  it("decrements remaining life by dt", () => {
    const [p] = stepParticles([spark({ life: 1, maxLife: 1 })], 0.25, { gravity: 0 });
    expect(p.life).toBeCloseTo(0.75);
  });

  it("removes sparks whose life has run out", () => {
    const alive = spark({ life: 1 });
    const dying = spark({ life: 0.1 });
    const next = stepParticles([alive, dying], 0.2, { gravity: 0 });
    expect(next).toHaveLength(1);
  });

  it("drains to empty when stepped forward past every lifetime", () => {
    let sparks = spawnBurst(0, 0, { count: 20, life: 1, rng: half });
    for (let i = 0; i < 60 && sparks.length > 0; i++) {
      sparks = stepParticles(sparks, 1 / 30, { gravity: 240 });
    }
    expect(sparks).toHaveLength(0);
  });

  it("does not mutate the input particles", () => {
    const input = [spark({ x: 1, y: 2, vx: 3, vy: 4, life: 1 })];
    const snapshot = { ...input[0] };
    stepParticles(input, 0.5, { gravity: 100 });
    expect(input[0]).toEqual(snapshot);
  });
});

describe("pick", () => {
  it("selects the element at rng * length", () => {
    expect(pick(["a", "b", "c", "d"], () => 0.5)).toBe("c"); // floor(0.5*4)=2
  });

  it("stays in bounds when rng returns its supremum", () => {
    expect(pick(["a", "b", "c"], () => 1)).toBe("a"); // floor(1*3)=3 -> %3 = 0
  });
});

describe("particleAlpha", () => {
  it("is fully opaque at the start of life", () => {
    expect(particleAlpha(spark({ life: 1.2, maxLife: 1.2 }))).toBe(1);
  });

  it("fades toward zero as life is spent", () => {
    expect(particleAlpha(spark({ life: 0.3, maxLife: 1.2 }))).toBeCloseTo(0.25);
  });

  it("never reports negative alpha for an expired spark", () => {
    expect(particleAlpha(spark({ life: -0.5, maxLife: 1.2 }))).toBe(0);
  });
});
