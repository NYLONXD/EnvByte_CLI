import { API_STATUS_URL, REPO_URL } from "../lib/content";
import { Wordmark } from "./Logo";

const LINKS = [
  { href: "/about", label: "About" },
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
        {/* The year is baked in when the page is prerendered; the browser may
            disagree right after New Year. */}
        <p className="sm:ml-auto" suppressHydrationWarning>
          © {new Date().getFullYear()} Envbyte, built by{" "}
          <a href="/about" className="text-fg hover:text-plain">
            Himanshu Jha
          </a>
        </p>
      </div>
    </footer>
  );
}
