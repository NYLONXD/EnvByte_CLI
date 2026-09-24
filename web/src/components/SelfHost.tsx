import { REPO_URL } from "../lib/content";

export function SelfHost() {
  return (
    <section id="self-host" className="scroll-mt-12 border-y border-line bg-sunken py-16 sm:py-24 lg:py-28">
      <div className="container-page grid items-center gap-10 lg:grid-cols-[0.9fr_1.1fr] lg:gap-16">
        <div>
          <p className="kicker" data-reveal>
            Self-host
          </p>
          <h2 className="text-[clamp(1.8rem,3.6vw,2.6rem)] leading-tight font-bold" data-reveal>
            Prefer your own server?
          </h2>
          <p className="mt-4.5 max-w-[36em] text-[1.06rem] text-muted" data-reveal>
            The hosted service works out of the box. If your company would rather run everything itself, the API ships
            as a small container with its own Postgres, and the CLI points at it with one variable.
          </p>
          <a
            href={`${REPO_URL}#run-the-complete-project-locally`}
            className="mt-5.5 inline-block font-semibold text-accent hover:underline"
            data-reveal
          >
            Self-hosting guide →
          </a>
        </div>
        <div className="min-w-0 rounded-2xl border border-white/10 bg-term" data-reveal>
          <pre className="overflow-x-auto px-4 py-5 text-[0.8rem] sm:px-6 sm:py-5.5 sm:text-[0.88rem] leading-[1.8] text-term-fg">
            <code>
              <span className="text-term-muted"># on your server</span>
              {"\ngit clone " + REPO_URL}
              {"\ncd EnvByte_CLI && cp .env.example .env"}
              {"\ndocker compose up -d\n\n"}
              <span className="text-term-muted"># on every laptop</span>
              {"\nexport ENVBYTE_SERVER=https://envbyte.your-company.com"}
            </code>
          </pre>
        </div>
      </div>
    </section>
  );
}
