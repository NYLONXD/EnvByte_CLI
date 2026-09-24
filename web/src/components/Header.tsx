import { REPO_URL } from "../lib/content";
import { Wordmark } from "./Logo";

const LINKS = [
  { href: "#how", label: "How it works" },
  { href: "#features", label: "Features" },
  { href: "#self-host", label: "Self-host" },
  { href: REPO_URL, label: "GitHub" },
];

export function Header() {
  return (
    <header className="sticky top-0 z-20 border-b border-line bg-page/80 backdrop-blur-md backdrop-saturate-150">
      <div className="container-page flex h-16 items-center gap-6">
        <a href="/" aria-label="Envbyte home" className="text-xl">
          <Wordmark />
        </a>
        <nav aria-label="Primary" className="ml-auto hidden gap-1 md:flex">
          {LINKS.map((link) => (
            <a
              key={link.href}
              href={link.href}
              className="rounded-lg px-3 py-2 text-[0.95rem] text-muted transition-colors hover:bg-accent-soft hover:text-fg"
            >
              {link.label}
            </a>
          ))}
        </nav>
        <a href="#install" className="button ml-auto px-3.5 py-2 text-sm md:ml-0">
          Install
        </a>
      </div>
    </header>
  );
}
