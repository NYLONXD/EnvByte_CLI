import { useRef } from "react";
import { TERMINAL_SCRIPT } from "../lib/content";
import { MOTION_OK, ScrollTrigger, gsap, useGSAP } from "../lib/gsap";

const TYPE_SECONDS_PER_CHAR = 0.032;

/**
 * An example session. The finished transcript is what renders; with motion
 * allowed, it is cleared and typed out once it scrolls into view.
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
        const cleared: [gsap.TweenTarget, gsap.TweenVars][] = [
          [lines, { autoAlpha: 0 }],
          ["[data-typed]", { text: "" }],
          ["[data-line-cursor]", { display: "none" }],
        ];
        if (finalCursor) cleared.push([finalCursor, { autoAlpha: 0 }]);

        // Cleared straight away, so the finished transcript never shows before
        // the typing starts, and again at the start of every replay.
        for (const [target, vars] of cleared) gsap.set(target, vars);
        const tl = gsap.timeline({ paused: true });
        for (const [target, vars] of cleared) tl.set(target, vars);
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

        ScrollTrigger.create({ trigger: scope.current, start: "top 75%", once: true, onEnter: () => tl.play() });
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
      className="min-w-0 overflow-hidden rounded-[1.25rem] border border-white/10 bg-term shadow-[0_40px_90px_-30px_rgb(0_0_0/0.6)]"
      aria-label="Example Envbyte session"
      role="figure"
    >
      <div className="flex items-center gap-3 border-b border-term-line bg-term-bar px-4 py-2.5">
        <p className="font-mono text-xs text-term-muted">~/payments-api</p>
        <button
          type="button"
          onClick={() => timeline.current?.restart()}
          className="ml-auto rounded-full border border-white/15 px-3 py-0.5 text-xs text-term-muted transition-colors hover:text-term-fg"
          aria-label="Replay the example session"
        >
          Replay
        </button>
      </div>
      <pre className="min-h-[24em] px-5 pt-4.5 pb-5.5 text-[clamp(0.72rem,1.4vw,0.8rem)] leading-7 whitespace-pre-wrap text-term-fg [overflow-wrap:anywhere]">
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
                      ? "text-term-fg/55"
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
