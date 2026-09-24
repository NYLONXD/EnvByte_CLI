import { REPO_URL } from "../lib/content";

export function CallToAction() {
  return (
    <section className="border-t border-line py-24 sm:py-32">
      <div className="container-page">
        <h2 className="max-w-[22ch] text-[clamp(1.9rem,4.6vw,3.3rem)] leading-[1.08]">Stop pasting secrets into chat.</h2>
        <div className="mt-10 flex flex-wrap items-center gap-3">
          <a href="#install" className="button">
            Install Envbyte
          </a>
          <a href={REPO_URL} className="button-ghost">
            View the source on GitHub
          </a>
        </div>
        <p className="mt-5 text-muted">Free and open source, under MIT or Apache-2.0.</p>
      </div>
    </section>
  );
}
