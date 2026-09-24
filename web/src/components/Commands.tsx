import type { ReactNode } from "react";

interface Entry {
  /** What you type. Each string is one line. */
  commands: string[];
  title: string;
  body: ReactNode;
}

// In the order a teammate meets them: joining, working, leaving.
const ENTRIES: Entry[] = [
  {
    commands: ["envbyte add priya@company.com"],
    title: "Invitations that can't be forwarded",
    body: "The project key is sealed to Priya's own identity. Forward the email to anyone else and it gives them nothing.",
  },
  {
    commands: ["envbyte logs --remote", "envbyte rollback --address <commit>"],
    title: "History and rollback",
    body: "Every push is a version. Roll the server, or only your local copy, back to any earlier one.",
  },
  {
    commands: ["envbyte role <user-id> viewer", "envbyte audit"],
    title: "Roles and an audit log",
    body: "Owner, admin, member and viewer. The log shows who pushed, pulled, invited and rotated, per project.",
  },
  {
    commands: ["ENVBYTE_IDENTITY_KEY=… envbyte pull"],
    title: "Made for CI",
    body: "Give your pipeline an identity of its own, and revoke it exactly like a teammate.",
  },
  {
    commands: ["envbyte remove <user-id>", "envbyte rotate"],
    title: "Offboarding in two commands",
    body: "Rotation mints a new project key and re-encrypts every file. The copy a departing member kept opens nothing new.",
  },
];

export function Commands() {
  return (
    <section id="commands" className="scroll-mt-12 border-t border-line bg-sunken py-20 sm:py-24 lg:py-32">
      <div className="container-page grid gap-10 lg:grid-cols-[minmax(0,4fr)_minmax(0,8fr)] lg:gap-16">
        <div>
          <h2 className="heading-2 max-w-[14ch] lg:sticky lg:top-28">From first invite to last day</h2>
        </div>

        <ul className="border-b border-line">
          {ENTRIES.map((entry) => (
            <li
              key={entry.title}
              className="grid gap-3 border-t border-line py-7 md:grid-cols-[minmax(0,5fr)_minmax(0,6fr)] md:gap-10"
            >
              <div className="order-2 md:order-1">
                <h3 className="text-[1.125rem]">{entry.title}</h3>
                <p className="mt-1.5 text-muted">{entry.body}</p>
              </div>
              <div className="order-1 grid content-start gap-1.5 md:order-2">
                {entry.commands.map((command) => (
                  <code key={command} className="font-mono text-[0.8rem] leading-6 break-all text-plain">
                    {command}
                  </code>
                ))}
              </div>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}
