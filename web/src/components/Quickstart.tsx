import { Terminal } from "./Terminal";

const STEPS = [
  { label: "Create your account", command: "envbyte register" },
  { label: "Start a project in your app's folder", command: "envbyte create payments-api" },
  { label: "Encrypt and upload your .env", command: 'envbyte push -m "first version"' },
  { label: "Invite a teammate", command: "envbyte add priya@company.com" },
  { label: "They join and pull it down", command: "envbyte init && envbyte pull" },
];

export function Quickstart() {
  return (
    <section id="quickstart" className="border-t border-line py-20 sm:py-24 lg:py-32">
      <div className="container-page grid items-start gap-12 lg:grid-cols-[minmax(0,0.95fr)_minmax(0,1.05fr)] lg:gap-16">
        <div className="min-w-0">
          <h2 className="heading-2">Up and running in a minute</h2>
          <p className="mt-5 max-w-[34em] text-muted">
            No keys to copy and no passwords to share. Your teammate signs in, accepts the invitation and pulls.
          </p>
          <ol className="mt-9 grid gap-5">
            {STEPS.map((step, index) => (
              <li key={step.command} className="grid grid-cols-[1.75rem_minmax(0,1fr)] gap-x-3">
                <span className="font-mono text-sm leading-7 text-muted">{index + 1}</span>
                <div className="min-w-0">
                  <p className="leading-7">{step.label}</p>
                  <code className="mt-1 block font-mono text-[0.8rem] break-all text-plain">{step.command}</code>
                </div>
              </li>
            ))}
          </ol>
        </div>
        <Terminal />
      </div>
    </section>
  );
}
