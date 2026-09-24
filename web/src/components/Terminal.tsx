import { useRef } from "react";
import { TERMINAL_SCRIPT } from "../lib/content";
import { MOTION_OK, gsap, useGSAP } from "../lib/gsap";

const TYPE_SECONDS_PER_CHAR = 0.032;

/**
 * The hero's example session. The finished transcript is what renders; with
 * motion allowed, GSAP clears it and replays it as if typed.
 */
export function Terminal() {
  const scope = useRef<HTMLDivElement>(null);
  const timeline = useRef<gsap.core.Timeline | null>(null);

  useGSAP(
    () => {
      const mm = gsap.matchMedia();
      mm.add(MOTION_OK, () => {
        const lines = gsap.utils.toArray<HTMLElement>("[data-line]");
        const finalCursor = scope.current?.querySelector<HTMLElement>("[data-final-cursor]");
        const tl = gsap.timeline({ delay: 1.1 });

        tl.set(lines, { autoAlpha: 0 });
        tl.set("[data-typed]", { text: "" });
        tl.set("[data-line-cursor]", { display: "none" });
        if (finalCursor) tl.set(finalCursor, { autoAlpha: 0 });

        for (const line of lines) {
          const typed = line.querySelector<HTMLElement>("[data-typed]");
          if (typed) {
            const text = typed.dataset.typed ?? "";
            const cursor = line.querySelector("[data-line-cursor]");
            tl.set(line, { autoAlpha: 1 });
            tl.set(cursor, { display: "inline-block" });
            tl.to({}, { duration: 0.35 });
            tl.to(typed, { text: { value: text }, duration: text.length * TYPE_SECONDS_PER_CHAR, ease: "none" });
            tl.to({}, { duration: 0.3 });
            tl.set(cursor, { display: "none" });
          } else {
            tl.fromTo(line, { autoAlpha: 0, y: 4 }, { autoAlpha: 1, y: 0, duration: 0.25, ease: "power2.out" });
            tl.to({}, { duration: 0.22 });
          }
        }
        if (finalCursor) tl.set(finalCursor, { autoAlpha: 1 });
        timeline.current = tl;
        return () => {
          timeline.current = null;
        };
      });
    },
    { scope },
  );

  return (
    <div
      ref={scope}
      data-animate="terminal"
      className="min-w-0 overflow-hidden rounded-2xl border border-white/10 bg-term shadow-[0_30px_80px_-20px_rgb(0_0_0/0.55)]"
      aria-label="Example Envbyte session"
      role="figure"
    >
      <div className="flex items-center gap-2 border-b border-white/8 bg-term-bar px-3.5 py-3">
        <span className="size-2.75 rounded-full bg-[#ff5f57]/80" />
        <span className="size-2.75 rounded-full bg-[#febc2e]/80" />
        <span className="size-2.75 rounded-full bg-[#28c840]/80" />
        <p className="mx-auto font-mono text-xs text-term-muted">~/payments-api</p>
        <button
          type="button"
          onClick={() => timeline.current?.restart()}
          className="rounded-md border border-white/15 px-2.5 py-0.5 text-xs text-term-muted transition-colors hover:text-term-fg"
          aria-label="Replay the example session"
        >
          Replay
        </button>
      </div>
      <pre className="min-h-[25em] px-5 whitespace-pre-wrap [overflow-wrap:anywhere] pt-4.5 pb-5.5 text-[clamp(0.76rem,1.5vw,0.87rem)] leading-7 text-term-fg">
        {TERMINAL_SCRIPT.map((line, index) => (
          <span key={index} data-line className="block">
            {line.kind === "command" ? (
              <>
                <span className="text-mint select-none">$ </span>
                <span data-typed={line.text}>{line.text}</span>
                <span data-line-cursor className="cursor hidden" aria-hidden="true" />
              </>
            ) : (
              <span
                className={
                  line.kind === "success"
                    ? "text-mint"
                    : line.kind === "comment"
                      ? "text-term-note"
                      : "text-term-muted"
                }
              >
                {line.text}
              </span>
            )}
          </span>
        ))}
        <span data-final-cursor className="block">
          <span className="text-mint select-none">$ </span>
          <span className="cursor" aria-hidden="true" />
        </span>
      </pre>
    </div>
  );
}
