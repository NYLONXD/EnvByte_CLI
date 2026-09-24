import { useRef } from "react";
import { CallToAction } from "./components/CallToAction";
import { Features } from "./components/Features";
import { Footer } from "./components/Footer";
import { Header } from "./components/Header";
import { Hero } from "./components/Hero";
import { HowItWorks } from "./components/HowItWorks";
import { Quickstart } from "./components/Quickstart";
import { SelfHost } from "./components/SelfHost";
import { MOTION_OK, ScrollTrigger, gsap, useGSAP } from "./lib/gsap";

export function App() {
  const main = useRef<HTMLElement>(null);

  // Anything marked data-reveal fades up as it scrolls into view, in batches
  // so a row of cards arrives with a short stagger rather than all at once.
  useGSAP(
    () => {
      const mm = gsap.matchMedia();
      mm.add(MOTION_OK, () => {
        gsap.set("[data-reveal]", { autoAlpha: 0, y: 24 });
        ScrollTrigger.batch("[data-reveal]", {
          start: "top 88%",
          once: true,
          onEnter: (batch) =>
            gsap.to(batch, { autoAlpha: 1, y: 0, duration: 0.7, stagger: 0.08, ease: "power3.out", overwrite: true }),
        });
      });
    },
    { scope: main },
  );

  return (
    <>
      <a
        href="#main"
        className="absolute -top-12 left-4 z-30 rounded-lg bg-mint px-3.5 py-2 font-semibold text-on-mint focus:top-3"
      >
        Skip to content
      </a>
      <Header />
      <main id="main" ref={main}>
        <Hero />
        <HowItWorks />
        <Features />
        <Quickstart />
        <SelfHost />
        <CallToAction />
      </main>
      <Footer />
    </>
  );
}
