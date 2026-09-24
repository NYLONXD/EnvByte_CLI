import type { ReactNode } from "react";

interface Feature {
  title: string;
  body: ReactNode;
  icon: ReactNode;
}

const FEATURES: Feature[] = [
  {
    title: "Offboarding in one command",
    body: (
      <>
        <code className="chip">envbyte remove</code> then <code className="chip">envbyte rotate</code> mints a new key
        and re-encrypts every file. The copy a departing member kept opens nothing new.
      </>
    ),
    icon: (
      <>
        <path d="M21 12a9 9 0 1 1-3-6.7" />
        <path d="M21 4v5h-5" />
      </>
    ),
  },
  {
    title: "History and rollback",
    body: "Every push is a version. Roll the server, or just your local copy, back to any earlier state.",
    icon: (
      <>
        <path d="M3 12a9 9 0 1 0 3-6.7L3 8" />
        <path d="M3 3v5h5" />
        <path d="M12 7v5l3 2" />
      </>
    ),
  },
  {
    title: "Roles and an audit log",
    body: "Owner, admin, member and viewer roles, plus a per-project log of who pushed, pulled, invited and rotated.",
    icon: (
      <>
        <circle cx="9" cy="8" r="3.5" />
        <path d="M2.5 20a6.5 6.5 0 0 1 13 0" />
        <path d="M16 4.5a3.5 3.5 0 0 1 0 7" />
        <path d="M18.5 14.5A6.5 6.5 0 0 1 21.5 20" />
      </>
    ),
  },
  {
    title: "Invitations that can't be forwarded",
    body: "An invite only works for the account it was sent to. Forwarding the email to someone else gives them nothing.",
    icon: (
      <>
        <rect x="3" y="5" width="18" height="14" rx="2" />
        <path d="m3 7 9 6 9-6" />
      </>
    ),
  },
  {
    title: "Made for CI",
    body: (
      <>
        Give your pipeline its own identity with <code className="chip">ENVBYTE_IDENTITY_KEY</code>, and revoke it
        exactly like a teammate.
      </>
    ),
    icon: (
      <>
        <path d="m8 9-4 3 4 3" />
        <path d="m16 9 4 3-4 3" />
        <path d="m13 6-2 12" />
      </>
    ),
  },
  {
    title: "Self-host in minutes",
    body: "The API and Postgres run from one Docker Compose file. Your server, your database, your rules.",
    icon: (
      <>
        <rect x="3" y="4" width="18" height="7" rx="1.5" />
        <rect x="3" y="13" width="18" height="7" rx="1.5" />
        <path d="M7 7.5h.01M7 16.5h.01" />
      </>
    ),
  },
];

export function Features() {
  return (
    <section id="features" className="scroll-mt-12 border-y border-line bg-sunken py-16 sm:py-24 lg:py-28">
      <div className="container-page">
        <p className="kicker" data-reveal>
          Features
        </p>
        <h2 className="max-w-[22em] text-[clamp(1.8rem,3.6vw,2.6rem)] leading-tight font-bold" data-reveal>
          Everything your team needs from its <span className="chip px-2 py-0 font-display text-[0.92em]">.env</span>
        </h2>
        <div className="mt-11 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {FEATURES.map((feature) => (
            <article
              key={feature.title}
              data-reveal
              className="card transition duration-200 hover:-translate-y-0.5 hover:border-mint/50"
            >
              <svg
                viewBox="0 0 24 24"
                aria-hidden="true"
                className="mb-4.5 size-10 rounded-[10px] bg-accent-soft fill-none stroke-accent p-2.25 [stroke-linecap:round] [stroke-linejoin:round] [stroke-width:1.8]"
              >
                {feature.icon}
              </svg>
              <h3 className="text-lg font-bold">{feature.title}</h3>
              <p className="mt-2.5 text-muted [&_code]:whitespace-normal">{feature.body}</p>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}
