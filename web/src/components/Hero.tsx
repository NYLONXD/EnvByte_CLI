import { useRef } from "react";
import { MOTION_OK, SplitText, gsap, useGSAP } from "../lib/gsap";
import { DownloadButton } from "./DownloadButton";
import { InstallBox } from "./InstallBox";
import { Terminal } from "./Terminal";

export function Hero() {
  const scope = useRef<HTMLElement>(null);

  useGSAP(
    () => {
      const mm = gsap.matchMedia();
      mm.add(MOTION_OK, () => {
        // Words rise out of a mask; SplitText labels the heading for screen
        // readers so they still hear one sentence.
        const split = SplitText.create("[data-animate='headline']", {
          type: "words",
          mask: "words",
          ignore: "[data-animate='badge']",
        });
        const tl = gsap.timeline({ defaults: { ease: "power3.out" } });
        tl.from("[data-animate='eyebrow']", { autoAlpha: 0, y: 12, duration: 0.5 })
          .from(split.words, { yPercent: 110, duration: 0.8, stagger: 0.06 }, "-=0.2")
          .from("[data-animate='badge']", { autoAlpha: 0, scale: 0.9, duration: 0.5 }, "<0.1")
          .from("[data-animate='lede']", { autoAlpha: 0, y: 16, duration: 0.6 }, "-=0.45")
          .from("[data-animate='install']", { autoAlpha: 0, y: 16, duration: 0.6 }, "-=0.4")
          .from("[data-animate='download']", { autoAlpha: 0, y: 16, duration: 0.6 }, "-=0.45")
          .from(
            "[data-animate='terminal']",
            { autoAlpha: 0, y: 32, scale: 0.97, duration: 0.9, ease: "expo.out" },
            "-=0.7",
          );
        // A slow drift keeps the glow from feeling static.
        gsap.to("[data-animate='glow']", {
          xPercent: -6,
          yPercent: 4,
          duration: 9,
          ease: "sine.inOut",
          yoyo: true,
          repeat: -1,
        });
        return () => split.revert();
      });
    },
    { scope },
  );

  return (
    <section ref={scope} className="dot-grid relative isolate overflow-hidden pt-14 pb-16 sm:pt-24 sm:pb-24 lg:pt-28">
      <div
        data-animate="glow"
        aria-hidden="true"
        className="pointer-events-none absolute -top-40 right-[-10%] -z-10 h-130 w-[70%] rounded-full bg-glow blur-3xl"
      />
      <div className="container-page grid items-center gap-12 lg:grid-cols-[1.05fr_1fr] lg:gap-16">
        <div className="min-w-0">
          <p
            data-animate="eyebrow"
            className="mb-5 inline-flex items-center gap-2.5 rounded-full border border-line-strong py-1.5 pr-3.5 pl-2.5 text-sm text-muted"
          >
            <span className="relative flex size-2">
              <span className="absolute inline-flex size-full animate-ping rounded-full bg-mint opacity-60 motion-reduce:hidden" />
              <span className="relative inline-flex size-2 rounded-full bg-mint" />
            </span>
            End-to-end encrypted · Open source
          </p>

          <h1 data-animate="headline" className="text-[clamp(2.4rem,6vw,4.1rem)] leading-[1.08] font-bold">
            Share{" "}
            <span data-animate="badge" className="chip inline-block px-2 py-0 font-display text-[0.92em]">
              .env
            </span>{" "}
            files.
            <br />
            <span className="text-accent">Never the secrets.</span>
          </h1>

          <p data-animate="lede" className="mt-6 mb-8 max-w-[34em] text-[clamp(1.04rem,1.6vw,1.16rem)] text-muted">
            Envbyte syncs, versions and rolls back your team's environment files. Everything is encrypted on your
            laptop before it leaves, every teammate gets their own sealed key, and the server can't read a single
            value.
          </p>

          <InstallBox />

          <div data-animate="download" className="mt-5">
            <DownloadButton />
          </div>
        </div>

        <Terminal />
      </div>
    </section>
  );
}
