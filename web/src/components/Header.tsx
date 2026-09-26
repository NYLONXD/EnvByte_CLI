import { REPO_URL } from "../lib/content";
import { Wordmark } from "./Logo";

export type PageId = "home" | "about";

// Root-relative, so the section links also work from the About page.
const LINKS = [
  { href: "/#how", label: "How it works" },
  { href: "/#commands", label: "Commands" },
  { href: "/#self-host", label: "Self-host" },
  { href: "/about", label: "About", page: "about" },
  { href: REPO_URL, label: "GitHub" },
];

/** A pill that floats over the top of the page, centred. */
export function Header({ current }: { current: PageId }) {
  return (
    <header className="pointer-events-none fixed inset-x-0 top-0 z-20 flex justify-center px-4 pt-3 sm:pt-4">
      <div className="pointer-events-auto flex items-center gap-1 rounded-full border border-line-strong bg-raised/85 py-1.5 pr-1.5 pl-4 shadow-[0_12px_32px_-12px_rgb(0_0_0/0.45)] backdrop-blur-md backdrop-saturate-150">
        <a href="/" aria-label="Envbyte home" aria-current={current === "home" ? "page" : undefined} className="mr-3 text-[1rem]">
          <Wordmark size={26} />
        </a>
        <nav aria-label="Primary" className="hidden md:flex">
          {LINKS.map((link) => {
            const active = link.page === current;
            return (
              <a
                key={link.href}
                href={link.href}
                aria-current={active ? "page" : undefined}
                className={`rounded-full px-3 py-1.5 text-[0.9rem] transition-colors lg:px-3.5 ${
                  active ? "text-fg" : "text-muted hover:text-fg"
                }`}
              >
                {link.label}
              </a>
            );
          })}
        </nav>
        <a href="/#install" className="button ml-1 px-4 py-2 text-sm">
          Install
        </a>
      </div>
    </header>
  );
}
