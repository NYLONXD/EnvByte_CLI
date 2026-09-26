import { Guilloche } from "./components/Guilloche";
import { Page } from "./components/Page";
import { AUTHOR_GITHUB_URL } from "./lib/content";

/**
 * Framed the way a banknote frames its portrait: an oval vignette with the
 * guilloché turning slowly behind it, rimmed in the two inks.
 */
function Portrait() {
  return (
    <div className="relative mx-auto w-full max-w-[17rem] sm:max-w-[20rem] md:max-w-[23rem]">
      <Guilloche className="pointer-events-none absolute top-1/2 left-1/2 -z-10 w-[200%] max-w-none -translate-1/2" />
      <div className="rounded-[50%] bg-linear-to-b from-copper to-mint p-[3px] shadow-[0_40px_90px_-30px_rgb(0_0_0/0.7)]">
        <picture>
          <source srcSet="/himanshu-jha.webp" type="image/webp" />
          <img
            src="/himanshu-jha.jpg"
            alt="Himanshu Jha"
            width={640}
            height={800}
            fetchPriority="high"
            className="aspect-[4/5] w-full rounded-[50%] bg-term object-cover"
          />
        </picture>
      </div>
    </div>
  );
}

export function AboutPage() {
  return (
    <Page current="about">
      <section className="relative isolate overflow-hidden pt-30 pb-20 sm:pt-36 sm:pb-24 lg:pt-40 lg:pb-32">
        <div className="container-page grid items-center gap-14 md:grid-cols-[minmax(0,5fr)_minmax(0,7fr)] md:gap-16 lg:gap-24">
          <Portrait />

          <div className="min-w-0">
            <h1 className="text-[clamp(2.05rem,4.8vw,3.4rem)] leading-[1.06]">Himanshu Jha</h1>
            <p className="mt-5 max-w-[30em] text-[clamp(1.1rem,1.6vw,1.25rem)] leading-relaxed">
              I build Envbyte, a way for teams to share .env files without ever sharing the secrets inside them.
            </p>

            <div className="mt-6 grid max-w-[34em] gap-4 text-muted">
              <p>
                It started with a problem most teams live with: API keys and database passwords get pasted into chat
                and email, and they stay in that history long after anyone needs them. I wanted sharing a .env file to
                be as easy as a git pull, with nobody in between able to read it.
              </p>
              <p>
                Envbyte is written in Rust, free to use, and open source under MIT or Apache-2.0. Companies that would
                rather keep everything in-house can run the whole thing on their own servers.
              </p>
              <p>If you run into a bug or have an idea, open an issue on GitHub.</p>
            </div>

            <div className="mt-9 flex flex-wrap items-center gap-3">
              <a href={AUTHOR_GITHUB_URL} className="button-ghost">
                <svg viewBox="0 0 24 24" className="size-4.5 fill-current" aria-hidden="true">
                  <path d="M12 .5a11.5 11.5 0 0 0-3.64 22.41c.58.1.79-.25.79-.56v-2c-3.2.7-3.88-1.37-3.88-1.37-.52-1.33-1.28-1.69-1.28-1.69-1.04-.71.08-.7.08-.7 1.16.08 1.77 1.19 1.77 1.19 1.03 1.76 2.7 1.25 3.36.96.1-.75.4-1.25.73-1.54-2.55-.29-5.24-1.28-5.24-5.68 0-1.26.45-2.28 1.18-3.09-.12-.29-.51-1.46.11-3.05 0 0 .97-.31 3.17 1.18a10.9 10.9 0 0 1 5.77 0c2.2-1.49 3.17-1.18 3.17-1.18.62 1.59.23 2.76.11 3.05.74.81 1.18 1.83 1.18 3.09 0 4.41-2.69 5.38-5.25 5.67.41.36.78 1.06.78 2.14v3.17c0 .31.21.67.8.56A11.5 11.5 0 0 0 12 .5Z" />
                </svg>
                @NYLONXD on GitHub
              </a>
              <a href="/#install" className="button">
                Install Envbyte
              </a>
            </div>
          </div>
        </div>
      </section>
    </Page>
  );
}
