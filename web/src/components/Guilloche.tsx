import { useSyncExternalStore } from "react";

/**
 * A guilloché rosette, the fine interlaced linework printed on banknotes and
 * certificates because it is hard to copy. Rings of waves in the two inks,
 * each ring a little out of phase with the last, so they weave into a lattice.
 */
const SIZE = 800;
const CENTER = SIZE / 2;
const RINGS = 22;
const LOBES = 18;
const STEPS = 360;

function ring(index: number, phase: number): string {
  const radius = 150 + index * 10;
  const amplitude = 16 + index * 0.35;
  let d = "";
  for (let step = 0; step <= STEPS; step++) {
    const theta = (step / STEPS) * Math.PI * 2;
    const r = radius + amplitude * Math.sin(LOBES * theta + index * 0.32 + phase);
    const x = CENTER + r * Math.cos(theta);
    const y = CENTER + r * Math.sin(theta);
    d += `${step === 0 ? "M" : "L"}${x.toFixed(1)} ${y.toFixed(1)}`;
  }
  return `${d}Z`;
}

const COPPER_RINGS = Array.from({ length: RINGS }, (_, index) => ring(index, 0));
const MINT_RINGS = Array.from({ length: RINGS }, (_, index) => ring(index, Math.PI));

function Layer({ rings, className }: { rings: string[]; className: string }) {
  return (
    <svg viewBox={`0 0 ${SIZE} ${SIZE}`} className={`absolute inset-0 size-full ${className}`}>
      <g fill="none" strokeWidth="0.7">
        {rings.map((d, index) => (
          <path key={index} d={d} />
        ))}
      </g>
    </svg>
  );
}

const subscribe = () => () => {};

/**
 * Each ink is its own layer and turns on the compositor, one a little slower
 * than the other, so the lattice slowly reweaves itself without repainting
 * any paths.
 *
 * The rings are drawn only in the browser: as prerendered markup their path
 * data would be most of the page, ahead of any of its words.
 */
export function Guilloche({ className = "" }: { className?: string }) {
  const inBrowser = useSyncExternalStore(
    subscribe,
    () => true,
    () => false,
  );
  return (
    <div
      aria-hidden="true"
      className={`aspect-square [mask-image:radial-gradient(closest-side,#000_35%,transparent)] ${className}`}
    >
      {inBrowser && (
        <div className="relative size-full motion-safe:animate-fade-in">
          <Layer rings={COPPER_RINGS} className="stroke-sealed opacity-32 motion-safe:animate-drift" />
          <Layer
            rings={MINT_RINGS}
            className="stroke-plain opacity-22 motion-safe:animate-drift [animation-duration:300s]"
          />
        </div>
      )}
    </div>
  );
}
