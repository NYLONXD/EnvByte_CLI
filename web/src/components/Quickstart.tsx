const COMMANDS = [
  { label: "Create your account", command: "envbyte register" },
  { label: "Start a project in your app's folder", command: "envbyte create payments-api" },
  { label: "Encrypt and upload your .env", command: 'envbyte push -m "first version"' },
  { label: "Invite a teammate", command: "envbyte add priya@company.com" },
  { label: "They join and pull it down", command: "envbyte init && envbyte pull" },
];

export function Quickstart() {
  return (
    <section id="quickstart" className="py-16 sm:py-24 lg:py-28">
      <div className="container-page grid items-center gap-10 lg:grid-cols-[0.9fr_1.1fr] lg:gap-16">
        <div>
          <p className="kicker" data-reveal>
            Quickstart
          </p>
          <h2 className="text-[clamp(1.8rem,3.6vw,2.6rem)] leading-tight font-bold" data-reveal>
            Up and running in a minute
          </h2>
          <p className="mt-4.5 max-w-[36em] text-[1.06rem] text-muted" data-reveal>
            Create an account, link a directory, push. Your teammate signs in, joins with their invitation and pulls. No
            keys to copy, no passwords to share.
          </p>
        </div>
        <ol className="grid gap-2.5">
          {COMMANDS.map((item, index) => (
            <li
              key={item.command}
              data-reveal
              className="grid gap-1.5 rounded-xl border border-line bg-raised px-4.5 py-3.5"
            >
              <span className="text-sm text-muted">
                <span className="font-mono text-accent">{index + 1}. </span>
                {item.label}
              </span>
              <code className="chip max-w-full justify-self-start overflow-x-auto whitespace-pre">{item.command}</code>
            </li>
          ))}
        </ol>
      </div>
    </section>
  );
}
