import { useEffect, useRef, useState, type ReactNode } from "react";
import { Page } from "./components/Page";
import { MACOS_PKG_URL, REPO_URL, SITE_URL, WINDOWS_MSI_URL } from "./lib/content";

/** The sidebar, in reading order. Each id is a section below. */
const CONTENTS = [
  {
    group: "Get started",
    sections: [
      { id: "install", title: "Install" },
      { id: "account", title: "Create an account" },
      { id: "first-project", title: "Your first project" },
      { id: "invite", title: "Invite your team" },
    ],
  },
  {
    group: "Everyday use",
    sections: [
      { id: "push-pull", title: "Push and pull" },
      { id: "conflicts", title: "When two people edit at once" },
      { id: "history", title: "History and rollback" },
    ],
  },
  {
    group: "Teams",
    sections: [
      { id: "roles", title: "Roles" },
      { id: "offboarding", title: "Removing someone" },
      { id: "second-computer", title: "A second computer" },
    ],
  },
  {
    group: "Reference",
    sections: [
      { id: "commands", title: "Commands" },
      { id: "settings", title: "Settings" },
      { id: "files", title: "Files on your computer" },
      { id: "self-host", title: "Self-hosting" },
      { id: "troubleshooting", title: "Troubleshooting" },
      { id: "uninstall", title: "Uninstall" },
    ],
  },
];

const SECTION_IDS = CONTENTS.flatMap((group) => group.sections.map((section) => section.id));

function Section({ id, title, children }: { id: string; title: string; children: ReactNode }) {
  return (
    <section id={id} className="scroll-mt-24 border-t border-line pt-10 pb-12 first:border-t-0 first:pt-0">
      <h2 className="text-[clamp(1.3rem,2.3vw,1.6rem)] leading-tight">
        <a href={`#${id}`} className="hover:text-plain">
          {title}
        </a>
      </h2>
      <div className="docs-prose mt-5">{children}</div>
    </section>
  );
}

/**
 * A terminal panel. Lines starting with `#` are comments, dimmed and left out
 * of what the copy button copies.
 */
function Code({ children }: { children: string }) {
  const [copied, setCopied] = useState(false);
  const timer = useRef<number | undefined>(undefined);
  useEffect(() => () => window.clearTimeout(timer.current), []);

  const lines = children.trim().split("\n");
  const commands = lines.filter((line) => line.trim() && !line.trim().startsWith("#")).join("\n");

  async function copy() {
    try {
      await navigator.clipboard.writeText(commands);
    } catch {
      return;
    }
    setCopied(true);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setCopied(false), 1600);
  }

  // The button sits beside the code, not over it, so a long line scrolls
  // without passing underneath.
  return (
    <div className="flex min-w-0 items-start gap-2 rounded-xl border border-white/10 bg-term py-2.5 pr-2.5 pl-4.5">
      <pre className="min-w-0 flex-1 overflow-x-auto py-1.5 text-[0.76rem] leading-[1.85] text-term-fg sm:text-[0.8rem]">
        <code>
          {lines.map((line, index) => (
            <span key={index} className={line.trim().startsWith("#") ? "text-term-muted" : undefined}>
              {line}
              {index < lines.length - 1 ? "\n" : ""}
            </span>
          ))}
        </code>
      </pre>
      <button
        type="button"
        onClick={copy}
        aria-label="Copy commands"
        className={`flex-none rounded-full border px-2.5 py-1 text-xs transition-colors ${
          copied ? "border-mint text-mint" : "border-white/15 text-term-fg hover:border-mint"
        }`}
      >
        {copied ? "Copied" : "Copy"}
      </button>
    </div>
  );
}

/** A remark set off from the text: `note` for tips, `warn` for what can go wrong. */
function Callout({ tone = "note", children }: { tone?: "note" | "warn"; children: ReactNode }) {
  const style = tone === "warn" ? "border-sealed bg-sealed-soft" : "border-plain bg-plain-soft";
  return <div className={`rounded-r-lg border-l-2 px-4 py-3 text-[0.95rem] ${style}`}>{children}</div>;
}

/**
 * A two-column table (a name and what it is) stacks into a list on a phone,
 * where side by side would squeeze the second column to a few letters.
 */
function Table({ head, rows }: { head: string[]; rows: ReactNode[][] }) {
  const stack = head.length === 2;
  return (
    <div className="overflow-x-auto">
      <table className={`w-full border-collapse text-left text-[0.92rem] ${stack ? "max-sm:block" : ""}`}>
        <thead className={stack ? "max-sm:hidden" : undefined}>
          <tr>
            {head.map((cell) => (
              <th key={cell} scope="col" className="border-b border-line-strong py-2 pr-5 font-semibold">
                {cell}
              </th>
            ))}
          </tr>
        </thead>
        <tbody className={stack ? "max-sm:block" : undefined}>
          {rows.map((row, index) => (
            <tr key={index} className={stack ? "max-sm:block max-sm:border-b max-sm:border-line max-sm:py-2.5" : undefined}>
              {row.map((cell, column) => (
                <td
                  key={column}
                  className={`border-b border-line py-2.5 pr-5 align-top text-muted first:text-fg ${
                    stack ? "max-sm:block max-sm:border-0 max-sm:py-0.5 max-sm:pr-0" : ""
                  }`}
                >
                  {cell}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/** Inline code. */
function C({ children }: { children: ReactNode }) {
  return <code className="chip">{children}</code>;
}

function Yes() {
  return (
    <>
      <span aria-hidden="true" className="text-plain">
        ✓
      </span>
      <span className="sr-only">Yes</span>
    </>
  );
}

function No() {
  return (
    <>
      <span aria-hidden="true">–</span>
      <span className="sr-only">No</span>
    </>
  );
}

/** The sidebar. `active` is the section in view, highlighted as you scroll. */
function Contents({ active }: { active: string }) {
  return (
    <div className="grid gap-6">
      {CONTENTS.map((group) => (
        <div key={group.group}>
          <p className="mb-2 font-mono text-[0.7rem] tracking-wider text-muted uppercase">{group.group}</p>
          <ul className="grid gap-0.5 border-l border-line">
            {group.sections.map((section) => {
              const current = section.id === active;
              return (
                <li key={section.id}>
                  <a
                    href={`#${section.id}`}
                    aria-current={current ? "location" : undefined}
                    className={`-ml-px block border-l py-1 pl-3.5 text-[0.9rem] transition-colors ${
                      current ? "border-plain text-fg" : "border-transparent text-muted hover:text-fg"
                    }`}
                  >
                    {section.title}
                  </a>
                </li>
              );
            })}
          </ul>
        </div>
      ))}
    </div>
  );
}

/** Follows the reader down the page, so the sidebar can say where they are. */
function useActiveSection(): string {
  const [active, setActive] = useState(SECTION_IDS[0]);
  useEffect(() => {
    const sections = SECTION_IDS.map((id) => document.getElementById(id)).filter(
      (element): element is HTMLElement => element !== null,
    );
    // The section whose top most recently crossed the upper third of the
    // screen is the one being read.
    const observer = new IntersectionObserver(
      (entries) => {
        const visible = entries.filter((entry) => entry.isIntersecting);
        if (visible.length) setActive(visible[0].target.id);
      },
      { rootMargin: "-15% 0px -70% 0px" },
    );
    sections.forEach((section) => observer.observe(section));
    return () => observer.disconnect();
  }, []);
  return active;
}

export function DocsPage() {
  const active = useActiveSection();

  return (
    <Page current="docs">
      <section className="pt-30 pb-10 sm:pt-36 lg:pt-40 lg:pb-14">
        <div className="container-page">
          <p className="font-mono text-sm text-plain">Docs</p>
          <h1 className="mt-3 text-[clamp(2.05rem,4.8vw,3.2rem)] leading-[1.06]">Using Envbyte</h1>
          <p className="mt-5 max-w-[38em] text-[clamp(1.06rem,1.5vw,1.15rem)] leading-relaxed text-muted">
            Install it, share a project with your team, and keep everyone's .env files in sync. Every command here
            runs in a terminal, inside the folder that holds your app's .env.
          </p>
        </div>
      </section>

      <div className="container-page grid gap-10 pb-24 lg:grid-cols-[13.5rem_minmax(0,1fr)] lg:gap-16">
        <aside className="min-w-0">
          <details className="rounded-2xl border border-line bg-raised px-4.5 py-3 lg:hidden">
            <summary className="cursor-pointer font-semibold">On this page</summary>
            <nav aria-label="On this page" className="mt-4 pb-1">
              <Contents active={active} />
            </nav>
          </details>
          <nav
            aria-label="On this page"
            className="sticky top-28 hidden max-h-[calc(100vh-8rem)] overflow-y-auto pb-6 lg:block"
          >
            <Contents active={active} />
          </nav>
        </aside>

        <article className="min-w-0 max-w-[46rem]">
          <Section id="install" title="Install">
            <p>Pick whichever suits you. They all install the same <C>envbyte</C> command.</p>
            <h3>macOS</h3>
            <p>
              Download <a href={MACOS_PKG_URL}>envbyte.pkg</a> and double-click it. It works on Apple silicon and
              Intel Macs and asks for your password, because it installs into <C>/usr/local/bin</C>. Or, in Terminal:
            </p>
            <Code>{`curl -fsSL ${SITE_URL}/install.sh | sh`}</Code>
            <Callout tone="warn">
              If macOS says it can't verify the developer, open <strong>System Settings → Privacy &amp; Security</strong>
              , scroll down, and click <strong>Open Anyway</strong> next to envbyte.pkg.
            </Callout>
            <h3>Windows</h3>
            <p>
              Download <a href={WINDOWS_MSI_URL}>envbyte.msi</a> and double-click it. It installs for your user only,
              so it needs no administrator rights. Or, in PowerShell or Command Prompt:
            </p>
            <Code>{`powershell -c "irm ${SITE_URL}/install.ps1 | iex"`}</Code>
            <Callout tone="warn">
              If Windows shows "Windows protected your PC", click <strong>More info</strong>, then{" "}
              <strong>Run anyway</strong>.
            </Callout>
            <h3>Linux</h3>
            <Code>{`curl -fsSL ${SITE_URL}/install.sh | sh`}</Code>
            <h3>npm or Cargo</h3>
            <Code>{`npm install -g envbyte
# or build it from source (Rust 1.88 or newer)
cargo install envbyte`}</Code>
            <p>Then open a new terminal and check it works:</p>
            <Code>{`envbyte --version`}</Code>
          </Section>

          <Section id="account" title="Create an account">
            <Code>{`envbyte register`}</Code>
            <p>
              It asks for your email, a username and a password, then emails you a verification code. Paste the code
              in and you're signed in. The code lasts two hours; if it expires or never arrives, check your spam
              folder, then run <C>envbyte verify</C> to get a new one.
            </p>
            <p>Other account commands:</p>
            <Table
              head={["Command", "What it does"]}
              rows={[
                [<C>envbyte login</C>, "Sign in on this computer"],
                [<C>envbyte whoami</C>, "Show who you're signed in as"],
                [<C>envbyte logout</C>, "Sign out on this computer"],
                [<C>envbyte reset-password</C>, "Email yourself a code and choose a new password"],
              ]}
            />
            <Callout tone="warn">
              Already use Envbyte on another computer? Read <a href="#second-computer">A second computer</a> before
              you run <C>envbyte login</C> on a new one.
            </Callout>
          </Section>

          <Section id="first-project" title="Your first project">
            <p>Go to the folder that holds your app's .env file, then:</p>
            <Code>{`envbyte create my-app
envbyte push -m "first version"`}</Code>
            <p>
              <C>create</C> links the folder to a new project and adds Envbyte's files and your .env files to{" "}
              <C>.gitignore</C>, so no secret ends up in git. Project names only have to be unique within your own
              account.
            </p>
            <p>
              <C>push</C> encrypts the file on your computer and uploads it. The server stores it but can't read it.
              If the folder has more than one .env file (<C>.env</C>, <C>.env.production</C>, ...), Envbyte asks which
              one, or you can name it:
            </p>
            <Code>{`envbyte push -f .env.production -m "add Stripe keys"`}</Code>
            <p>
              <C>envbyte status</C> shows which project the folder is linked to, who you're signed in as, and whether
              everything is in order.
            </p>
            <Callout>
              The first time you use Envbyte, it creates a personal key for this computer at{" "}
              <C>~/.envbyte-identity</C>. It's what lets this computer open your projects. Back it up like an SSH key,
              and never share it.
            </Callout>
          </Section>

          <Section id="invite" title="Invite your team">
            <ol>
              <li>
                Your teammate installs Envbyte and runs <C>envbyte register</C>. They need to be signed in at least
                once before you can invite them.
              </li>
              <li>You invite them by email:</li>
            </ol>
            <Code>{`envbyte add priya@company.com`}</Code>
            <ol start={3}>
              <li>
                They get an email with a one-time code. In their copy of the app's folder, they run{" "}
                <C>envbyte init</C>, paste the code, and then <C>envbyte pull</C>.
              </li>
            </ol>
            <Code>{`envbyte init
envbyte pull`}</Code>
            <p>
              That's all. Nobody sends a password or a key over chat. The invitation works only for the account it
              was sent to, so forwarding the email to someone else gives them nothing. It expires after 24 hours and
              can be used once.
            </p>
            <p>
              New people join as <strong>members</strong>. See <a href="#roles">Roles</a> to change that. Each
              teammate runs <C>envbyte init</C> in their own copy of the project, because the link file isn't
              committed to git.
            </p>
          </Section>

          <Section id="push-pull" title="Push and pull">
            <p>Push uploads your local file as a new version. Pull downloads the latest version.</p>
            <Code>{`envbyte push -m "rotate database password"
envbyte pull`}</Code>
            <p>
              Both take <C>-f &lt;file&gt;</C> to pick a file without being asked. Each push records a message, so
              history explains itself later.
            </p>
            <p>
              If your local file has no changes of its own, <C>pull</C> simply updates it. If it does, <C>pull</C>{" "}
              doesn't throw them away: it offers to merge them with the server's version (see the next section), to
              replace them, or to stop.
            </p>
          </Section>

          <Section id="conflicts" title="When two people edit at once">
            <p>
              Envbyte remembers which version of each file you last pulled or pushed. If a teammate pushed after that,
              your push isn't allowed to quietly overwrite their change. Envbyte merges the two instead, one variable
              at a time:
            </p>
            <ul>
              <li>A variable only you changed keeps your value.</li>
              <li>A variable only they changed takes theirs.</li>
              <li>Variables either of you added or removed are added or removed.</li>
              <li>
                A variable you <em>both</em> changed, to different values, is the only thing you're asked about.
              </li>
            </ul>
            <Code>{`$ envbyte push -m "new Redis host"
! .env changed on the server since this checkout last synced it.
--- Merging .env
  - API_KEY changed on the server
  - REDIS_URL changed by you

! DB_PASSWORD was changed both by you and on the server.
Which value should win?
  [1] keep yours         s3c...42 (24 chars)
  [2] take the server's  pr0...9f (24 chars)
Select (1-2): 1
Save the merged .env and push it? [y/N]: y
OK Pushed.`}</Code>
            <p>
              The merged file is saved to your folder and pushed, so you and your teammates end up with the same
              thing. Envbyte only shows the start and end of each value, never the whole secret.
            </p>
            <p>To make one side win outright instead:</p>
            <Code>{`# replace the server's copy with yours
envbyte push --force
# replace your copy with the server's
envbyte pull --force`}</Code>
            <Callout>
              Merging keeps your file's layout and comments. A change someone made only to a comment isn't carried
              over. In a script, with no one to answer, a merge without clashes goes ahead on its own and a real clash
              stops with an error naming the variables.
            </Callout>
            <Callout tone="warn">
              This needs Envbyte 0.5 or newer on every computer. Older versions overwrite whatever is on the server, so
              ask your team to update.
            </Callout>
          </Section>

          <Section id="history" title="History and rollback">
            <p>Every push is kept as a version. To see them, and to make an older one current again for everyone:</p>
            <Code>{`envbyte logs --remote
envbyte rollback --address <commit>
envbyte pull`}</Code>
            <p>
              Rollback asks before it changes anything. Teammates get the restored version on their next pull.
            </p>
            <h3>Local snapshots</h3>
            <p>
              You can also save a snapshot of <C>.env</C> in this folder only, without touching the server, and restore
              it later:
            </p>
            <Code>{`envbyte commit "before the migration"
envbyte logs
envbyte rollback --local <id>`}</Code>
            <p>Snapshots are stored encrypted, so restoring one needs you to be signed in.</p>
          </Section>

          <Section id="roles" title="Roles">
            <Table
              head={["", "Owner", "Admin", "Member", "Viewer"]}
              rows={[
                ["Pull files, see history and members", <Yes />, <Yes />, <Yes />, <Yes />],
                ["Push, roll back", <Yes />, <Yes />, <Yes />, <No />],
                ["Invite, remove, rotate the key, read the audit log", <Yes />, <Yes />, <No />, <No />],
                ["Make someone an admin, or remove an admin", <Yes />, <No />, <No />, <No />],
              ]}
            />
            <p>
              The person who creates a project is its owner. Find people's IDs with <C>envbyte members</C>, then:
            </p>
            <Code>{`envbyte members
envbyte role <user-id> viewer
envbyte audit`}</Code>
            <p>
              <C>envbyte audit</C> lists who pushed, rolled back, invited, removed and rotated, with dates.
            </p>
          </Section>

          <Section id="offboarding" title="Removing someone">
            <Code>{`envbyte members
envbyte remove <user-id>
envbyte rotate`}</Code>
            <p>
              <C>remove</C> cuts their access to the project straight away. <C>rotate</C> then gives the project a new
              key, re-locks every file with it, and hands it only to the people who remain, so anything the person
              kept from before can't open what you push next. It shows exactly what it will do and asks first;{" "}
              <C>rotate -y</C> skips the question.
            </p>
            <p>The rest of the team doesn't need to do anything. Their next pull or push picks up the new key.</p>
            <Callout tone="warn">
              Rotating protects what you push from now on. The person who left may have seen the actual values, so
              also change the passwords and API keys that matter, and push the new ones.
            </Callout>
          </Section>

          <Section id="second-computer" title="A second computer">
            <p>
              Your access lives in the personal key on your first computer. To use the same account on another one,
              take that key with you:
            </p>
            <ol>
              <li>
                On the computer you already use, run <C>envbyte identity export</C>. It prints your key. Treat it like
                a password.
              </li>
              <li>
                On the new computer, store it in the <C>ENVBYTE_IDENTITY_KEY</C> environment variable, for example in
                your shell profile on macOS and Linux, or in your user environment variables on Windows. Then open a
                new terminal and run <C>envbyte login</C>.
              </li>
              <li>
                Copy the <C>.envbyte</C> file from the project folder on your old computer into the same folder on the
                new one, then run <C>envbyte pull</C>. That file only says which project the folder belongs to; it
                contains no keys.
              </li>
            </ol>
            <Code>{`# macOS and Linux: add this line to ~/.zshrc or ~/.bashrc
export ENVBYTE_IDENTITY_KEY="<the key you exported>"`}</Code>
            <Callout tone="warn">
              If you sign in on a new computer without doing this, Envbyte creates a new key for your account. The new
              computer can't open your existing projects, and the old one loses access at the next key rotation. To
              recover, ask an admin of each project to run <C>envbyte rotate</C>; your new computer can then read the
              current files.
            </Callout>
          </Section>

          <Section id="commands" title="Commands">
            <Table
              head={["Command", "What it does"]}
              rows={[
                [<C>register</C>, "Create an account"],
                [<C>verify</C>, "Send a new verification code and finish signing up"],
                [<C>login</C>, "Sign in on this computer"],
                [<C>logout</C>, "Sign out on this computer"],
                [<C>whoami</C>, "Show the signed-in account"],
                [<C>reset-password</C>, "Choose a new password"],
                [<C>create &lt;name&gt;</C>, "Start a project in this folder"],
                [<C>init</C>, "Join a project you were invited to"],
                [<C>projects</C>, "List the projects you can reach"],
                [<C>push [-m msg] [-f file] [--force]</C>, "Encrypt and upload a .env file"],
                [<C>pull [-f file] [--force]</C>, "Download the latest version"],
                [<C>commit &lt;message&gt;</C>, "Save a local snapshot of .env"],
                [<C>logs</C>, "List local snapshots"],
                [<C>logs --remote</C>, "List every pushed version"],
                [<C>rollback --address &lt;commit&gt;</C>, "Make an older version current for everyone"],
                [<C>rollback --local &lt;id&gt;</C>, "Restore a local snapshot into .env"],
                [<C>add &lt;email&gt;</C>, "Invite someone"],
                [<C>members</C>, "List members, their roles and IDs"],
                [<C>role &lt;user-id&gt; &lt;role&gt;</C>, "Make someone an admin, member or viewer"],
                [<C>remove &lt;user-id&gt;</C>, "Remove someone from the project"],
                [<C>rotate [-y]</C>, "Give the project a new key, for current members only"],
                [<C>status</C>, "Show what this folder is linked to and whether it works"],
                [<C>audit</C>, "Show the project's audit log"],
                [<C>identity</C>, "Show this computer's key fingerprint"],
                [<C>identity export</C>, "Print this computer's key, for another computer"],
              ]}
            />
            <p>
              Every command starts with <C>envbyte</C>, and <C>envbyte help &lt;command&gt;</C> explains its options.
            </p>
          </Section>

          <Section id="settings" title="Settings">
            <p>Envbyte reads these environment variables. None are needed for normal use.</p>
            <Table
              head={["Variable", "What it's for"]}
              rows={[
                [<C>ENVBYTE_SERVER</C>, "Use your own Envbyte server instead of the hosted one"],
                [<C>ENVBYTE_IDENTITY_KEY</C>, "Your personal key, given directly instead of read from a file"],
                [<C>ENVBYTE_IDENTITY_FILE</C>, "Where your personal key is stored (default ~/.envbyte-identity)"],
              ]}
            />
          </Section>

          <Section id="files" title="Files on your computer">
            <Table
              head={["File", "What it holds"]}
              rows={[
                [<C>.envbyte</C>, "Which project this folder is linked to, and which versions you last synced"],
                [<C>.envbyte-logs</C>, "Your local snapshots, encrypted"],
                [<C>~/.envbyte-auth</C>, "Your sign-in on this computer"],
                [<C>~/.envbyte-identity</C>, "This computer's personal key. Back it up, never share it"],
              ]}
            />
            <p>
              The ones in your project folder, and your .env files, are added to <C>.gitignore</C> for you. On macOS
              and Linux only your user can read them.
            </p>
          </Section>

          <Section id="self-host" title="Self-hosting">
            <p>
              The hosted service works out of the box. To run your own server instead, you need Docker on a machine
              your team can reach over HTTPS:
            </p>
            <Code>{`git clone ${REPO_URL}
cd EnvByte_CLI
# set POSTGRES_PASSWORD and ENVBYTE_JWT_SECRET in .env
cp .env.example .env
docker compose up -d
curl http://localhost:3030/health/ready`}</Code>
            <p>Before your team uses it:</p>
            <ul>
              <li>Put it behind HTTPS. Envbyte refuses to send sign-ins over plain HTTP to anything but localhost.</li>
              <li>
                Set <C>ENVBYTE_ENV=production</C> and configure real email with <C>ENVBYTE_EMAIL_MODE=smtp</C> and the{" "}
                <C>SMTP_*</C> settings, so verification codes and invitations are delivered.
              </li>
              <li>
                Behind a load balancer or reverse proxy, set <C>ENVBYTE_TRUSTED_PROXY_HOPS</C> to the number of proxies
                (usually <C>1</C>), or everyone shares one rate limit.
              </li>
            </ul>
            <p>Then everyone points their CLI at it:</p>
            <Code>{`export ENVBYTE_SERVER=https://envbyte.your-company.com`}</Code>
            <p>
              Accounts on your server are separate from the hosted service. Every setting is described in the{" "}
              <a href={`${REPO_URL}/blob/main/server/README.md`}>server guide</a>.
            </p>
          </Section>

          <Section id="troubleshooting" title="Troubleshooting">
            <h3>"envbyte: command not found"</h3>
            <p>
              Open a new terminal; the installer added Envbyte to your PATH, and terminals that were already open
              don't know yet.
            </p>
            <h3>The first command is slow, or times out</h3>
            <p>
              The hosted server sleeps when nobody has used it for a while, and can take up to a minute to wake up. Run
              the command again.
            </p>
            <h3>"that account has not published an identity key yet"</h3>
            <p>
              The person you're inviting hasn't signed in with a current version of Envbyte. Ask them to run{" "}
              <C>envbyte login</C>, then invite them again.
            </p>
            <h3>"No .envbyte file found"</h3>
            <p>
              You're not in a linked folder. Go to your project's folder, or link it with <C>envbyte create</C> or{" "}
              <C>envbyte init</C>.
            </p>
            <h3>"... encrypted with project key version 1, but the project is on version 2"</h3>
            <p>
              The project's key was rotated and you don't have the new one. Run <C>envbyte status</C> to confirm, then
              ask an admin to run <C>envbyte rotate</C>, which hands the current key to every member.
            </p>
            <h3>"Both you and the server changed ..."</h3>
            <p>
              You pushed from a script, and a teammate changed the same variable. Run the push in a terminal to choose,
              or use <C>--force</C> to keep yours. See <a href="#conflicts">When two people edit at once</a>.
            </p>
            <h3>You lost your personal key</h3>
            <p>
              Sign in again with <C>envbyte login</C>, which creates a new key, then ask an admin of each project to
              run <C>envbyte rotate</C>. Versions pushed before that may stay unreadable to you.
            </p>
            <p>
              Something else? <a href={`${REPO_URL}/issues`}>Open an issue on GitHub</a>.
            </p>
          </Section>

          <Section id="uninstall" title="Uninstall">
            <Table
              head={["Installed with", "To remove it"]}
              rows={[
                [
                  "envbyte.pkg (macOS)",
                  <C>sudo rm /usr/local/bin/envbyte && sudo pkgutil --forget in.trackedge.envbyte</C>,
                ],
                ["envbyte.msi (Windows)", "Settings → Apps → Envbyte → Uninstall"],
                ["install.sh", <C>rm -rf ~/.envbyte</C>],
                ["install.ps1", "Delete the .envbyte folder in your user folder"],
                ["npm", <C>npm uninstall -g envbyte</C>],
                ["Cargo", <C>cargo uninstall envbyte</C>],
              ]}
            />
            <p>
              Uninstalling keeps your sign-in and personal key, in case you come back. If you're done for good, run{" "}
              <C>envbyte logout</C> first, then delete <C>~/.envbyte-identity</C>.
            </p>
          </Section>
        </article>
      </div>
    </Page>
  );
}
