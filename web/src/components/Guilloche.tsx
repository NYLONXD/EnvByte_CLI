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

export function Guilloche({ className = "" }: { className?: string }) {
  return (
    <svg viewBox={`0 0 ${SIZE} ${SIZE}`} className={className} aria-hidden="true">
      <defs>
        <radialGradient id="guilloche-fade">
          <stop offset="0.35" stopColor="white" stopOpacity="1" />
          <stop offset="1" stopColor="white" stopOpacity="0" />
        </radialGradient>
        <mask id="guilloche-mask">
          <rect width={SIZE} height={SIZE} fill="url(#guilloche-fade)" />
        </mask>
      </defs>
      <g mask="url(#guilloche-mask)" fill="none" strokeWidth="0.7">
        <g className="stroke-sealed" opacity="0.32">
          {COPPER_RINGS.map((d, index) => (
            <path key={index} d={d} />
          ))}
        </g>
        <g className="stroke-plain" opacity="0.22">
          {MINT_RINGS.map((d, index) => (
            <path key={index} d={d} />
          ))}
        </g>
      </g>
    </svg>
  );
}
