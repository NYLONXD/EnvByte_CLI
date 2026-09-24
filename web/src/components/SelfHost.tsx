import { REPO_URL } from "../lib/content";

export function SelfHost() {
  return (
    <section id="self-host" className="scroll-mt-12 border-t border-line bg-sunken py-20 sm:py-24 lg:py-32">
      <div className="container-page grid items-center gap-12 lg:grid-cols-[minmax(0,0.95fr)_minmax(0,1.05fr)] lg:gap-16">
        <div>
          <h2 className="heading-2">Prefer your own server?</h2>
          <p className="mt-5 max-w-[34em] text-muted">
            The hosted service works out of the box. If your company would rather run everything itself, the API ships
            as a small container with its own Postgres, and the CLI points at it with one variable.
          </p>
          <a
            href={`${REPO_URL}#run-the-complete-project-locally`}
            className="mt-6 inline-block font-semibold text-plain underline decoration-plain/35 underline-offset-4 hover:decoration-plain"
          >
            Read the self-hosting guide
          </a>
        </div>
        <div className="min-w-0 rounded-[1.25rem] border border-white/10 bg-term">
          <pre className="overflow-x-auto px-5 py-5.5 text-[0.74rem] leading-[1.9] text-term-fg sm:px-6 sm:text-[0.8rem]">
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
