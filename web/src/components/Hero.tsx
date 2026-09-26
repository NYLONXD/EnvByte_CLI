import { useRef } from "react";
import { MOTION_OK, gsap, useGSAP } from "../lib/gsap";
import { DownloadButton } from "./DownloadButton";
import { EnvLens } from "./EnvLens";
import { Guilloche } from "./Guilloche";
import { InstallBox } from "./InstallBox";

export function Hero() {
  const scope = useRef<HTMLElement>(null);

  // The page's one entrance: the headline and install settle while the lens
  // seals its file (EnvLens runs that part).
  useGSAP(
    () => {
      const mm = gsap.matchMedia();
      mm.add(MOTION_OK, () => {
        gsap
          .timeline({ defaults: { ease: "power3.out" } })
          .from("[data-animate='headline']", { autoAlpha: 0, y: 14, duration: 0.8 })
          .from("[data-animate='intro']", { autoAlpha: 0, y: 14, duration: 0.7 }, "-=0.5")
          .from("[data-animate='lens']", { autoAlpha: 0, y: 24, duration: 0.9 }, "<");
      });
    },
    { scope },
  );

  return (
    <section ref={scope} className="relative isolate overflow-hidden pt-30 pb-18 sm:pt-36 sm:pb-22 lg:pt-40 lg:pb-26">
      <Guilloche className="pointer-events-none absolute top-[-2rem] right-[-16rem] -z-10 w-[62rem] max-w-none opacity-80 sm:top-0 lg:right-[-12rem]" />
      <div className="container-page">
        <h1
          data-animate="headline"
          className="max-w-[18ch] text-[clamp(2.05rem,5.6vw,4rem)] leading-[1.06]"
        >
          Share .env files. Never the secrets.
        </h1>

        <div className="mt-10 grid items-start gap-12 lg:mt-14 lg:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)] lg:gap-14">
          <div data-animate="intro" className="min-w-0">
            <p className="mb-8 max-w-[34em] text-[clamp(1.06rem,1.5vw,1.18rem)] leading-relaxed text-muted">
              Envbyte syncs, versions and rolls back your team's environment files. The file is encrypted on your
              laptop before it leaves, only the teammates you invite can open it, and the server can't read a single
              value.
            </p>
            <InstallBox />
            <div className="mt-6">
              <DownloadButton />
            </div>
          </div>

          <EnvLens />
        </div>
      </div>
    </section>
  );
}
