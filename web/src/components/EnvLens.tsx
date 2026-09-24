import { useRef, type CSSProperties } from "react";
import { ENV_SAMPLE } from "../lib/content";
import { MOTION_OK, gsap, useGSAP } from "../lib/gsap";

const RESTING_SPLIT = 46;

/**
 * Stand-in ciphertext, shaped like the file so the two layers line up. It is
 * base64 like the real envelope, from a fixed seed so it never shifts.
 */
function toCiphertext(lines: string[]): string[] {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  let seed = 0x2f6b1c3d;
  const next = () => {
    seed ^= seed << 13;
    seed ^= seed >>> 17;
    seed ^= seed << 5;
    return alphabet[(seed >>> 0) % alphabet.length];
  };
  const cipher = lines.map((line) => Array.from(line, next).join(""));
  const last = cipher.length - 1;
  cipher[last] = cipher[last].slice(0, -2) + "==";
  return cipher;
}

const CIPHERTEXT = toCiphertext(ENV_SAMPLE);

function PlainLine({ line }: { line: string }) {
  if (line.startsWith("#")) return <span className="text-term-muted">{line}</span>;
  const split = line.indexOf("=");
  return (
    <>
      <span className="text-term-fg">{line.slice(0, split)}</span>
      <span className="text-term-muted">=</span>
      <span className="text-mint">{line.slice(split + 1)}</span>
    </>
  );
}

/**
 * The hero: one .env file with a divider through it. Left of the divider is
 * the file as your team reads it; right of it, what the server stores. Drag
 * it, or focus it and use the arrow keys.
 */
export function EnvLens() {
  const stage = useRef<HTMLDivElement>(null);
  const range = useRef<HTMLInputElement>(null);
  const sweep = useRef<gsap.core.Tween | null>(null);

  function show(split: number) {
    stage.current?.style.setProperty("--split", `${split}%`);
    if (range.current) range.current.value = String(split);
  }

  // On load the divider starts at the far right, the whole file readable,
  // and sweeps across as if the file were being sealed.
  useGSAP(
    () => {
      const mm = gsap.matchMedia();
      mm.add(MOTION_OK, () => {
        const state = { split: 100 };
        show(100);
        sweep.current = gsap.to(state, {
          split: RESTING_SPLIT,
          duration: 1.9,
          delay: 0.7,
          ease: "power3.inOut",
          onUpdate: () => show(Math.round(state.split * 10) / 10),
        });
      });
    },
    { scope: stage },
  );

  const layer = "px-4 pt-12 pb-5 whitespace-pre-wrap break-all sm:px-5.5";

  return (
    <figure
      data-animate="lens"
      className="min-w-0 overflow-hidden rounded-[1.25rem] border border-white/10 bg-term shadow-[0_40px_90px_-30px_rgb(0_0_0/0.7)]"
    >
      <div
        ref={stage}
        className="relative font-mono text-[0.74rem] leading-[2.05] select-none sm:text-[0.8rem]"
        style={{ "--split": `${RESTING_SPLIT}%` } as CSSProperties}
      >
        <pre className={layer}>
          <span className="absolute top-4 left-4 flex items-center gap-2 text-[0.72rem] font-medium text-mint sm:left-5.5">
            <span className="size-1.5 rounded-full bg-mint" />
            Your team sees
          </span>
          {ENV_SAMPLE.map((line) => (
            <span key={line} className="block">
              <PlainLine line={line} />
            </span>
          ))}
        </pre>

        <pre
          aria-hidden="true"
          className={`${layer} absolute inset-0 bg-term-sealed text-copper/85 [clip-path:inset(0_0_0_var(--split))]`}
        >
          <span className="absolute top-4 right-4 flex items-center gap-2 text-[0.72rem] font-medium text-copper sm:right-5.5">
            The server stores
            <span className="size-1.5 rounded-full bg-copper" />
          </span>
          {CIPHERTEXT.map((line, index) => (
            <span key={index} className="block">
              {line}
            </span>
          ))}
        </pre>

        <input
          ref={range}
          type="range"
          min={0}
          max={100}
          step={1}
          defaultValue={RESTING_SPLIT}
          onPointerDown={() => sweep.current?.kill()}
          onKeyDown={() => sweep.current?.kill()}
          onInput={(event) => show(Number(event.currentTarget.value))}
          aria-label="Divider between the file as your team sees it and as the server stores it"
          className="lens-range absolute inset-0 z-10 h-full w-full cursor-ew-resize opacity-0"
        />

        {/* Copper into mint, like the colour-shifting ink it is named after. */}
        <div
          aria-hidden="true"
          className="lens-divider pointer-events-none absolute inset-y-0 left-(--split) w-0.5 -translate-x-1/2 bg-linear-to-b from-copper via-copper/70 to-mint"
        >
          <span className="lens-handle absolute top-1/2 left-1/2 grid size-10 -translate-1/2 place-items-center rounded-full bg-linear-to-b from-copper to-mint p-0.5 shadow-[0_6px_20px_rgb(0_0_0/0.5)]">
            <span className="grid size-full place-items-center rounded-full bg-term">
              <svg viewBox="0 0 64 64" className="size-4.5" aria-hidden="true">
                <circle cx="32" cy="22" r="11" className="fill-term-fg" />
                <path d="M26.9 27.8h10.2l3.5 17c.35 1.85-1 3.2-2.9 3.2H26.3c-1.85 0-3.25-1.35-2.9-3.2z" className="fill-term-fg" />
              </svg>
            </span>
          </span>
        </div>
      </div>

      <figcaption className="flex flex-wrap items-center justify-between gap-x-4 gap-y-1 border-t border-term-line px-4 py-3 text-[0.82rem] text-term-muted sm:px-5.5">
        <span>
          <span className="font-mono text-term-fg [font-stretch:87.5%]">payments-api/.env</span>, as it leaves your laptop
        </span>
        <span>Drag the divider</span>
      </figcaption>
    </figure>
  );
}
