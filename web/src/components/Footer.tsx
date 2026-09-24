import { API_STATUS_URL, REPO_URL } from "../lib/content";
import { Wordmark } from "./Logo";

const LINKS = [
  { href: REPO_URL, label: "GitHub" },
  { href: `${REPO_URL}/blob/main/SECURITY.md`, label: "Security" },
  { href: `${REPO_URL}/releases`, label: "Releases" },
  { href: API_STATUS_URL, label: "API status" },
];

export function Footer() {
  return (
    <footer className="border-t border-line py-9 text-sm text-muted">
      <div className="container-page flex flex-wrap items-center gap-x-8 gap-y-4">
        <a href="/" className="text-fg">
          <Wordmark size={22} />
        </a>
        <nav aria-label="Footer" className="flex flex-wrap gap-x-5 gap-y-2">
          {LINKS.map((link) => (
            <a key={link.label} href={link.href} className="hover:text-fg">
              {link.label}
            </a>
          ))}
        </nav>
        <p className="sm:ml-auto">© {new Date().getFullYear()} Envbyte</p>
      </div>
    </footer>
  );
}
