import { REPO_URL } from "../lib/content";
import { LogoMark } from "./Logo";

export function CallToAction() {
  return (
    <section className="bg-[radial-gradient(50%_60%_at_50%_100%,var(--glow),transparent_70%)] py-18 sm:py-28">
      <div className="container-page grid justify-items-center text-center">
        <div data-reveal>
          <LogoMark size={56} className="mb-6" />
        </div>
        <h2 className="max-w-[16em] text-[clamp(1.8rem,3.6vw,2.6rem)] leading-tight font-bold" data-reveal>
          Stop pasting secrets into chat.
        </h2>
        <p className="mt-3.5 mb-7.5 text-[1.08rem] text-muted" data-reveal>
          Free, open source, and installed in one line.
        </p>
        <div className="flex flex-wrap justify-center gap-3" data-reveal>
          <a href="#install" className="button">
            Install Envbyte
          </a>
          <a href={REPO_URL} className="button-ghost">
            View on GitHub
          </a>
        </div>
      </div>
    </section>
  );
}
