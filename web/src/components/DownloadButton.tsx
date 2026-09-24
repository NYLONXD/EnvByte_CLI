import { WINDOWS_SETUP_URL } from "../lib/content";

/**
 * Downloads envbyte-setup.exe, which opens a terminal window, says what it will
 * install, asks, then installs the CLI and adds it to PATH.
 */
export function DownloadButton() {
  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
      <a href={WINDOWS_SETUP_URL} className="button">
        <svg viewBox="0 0 24 24" className="size-4.5 fill-none stroke-current stroke-[2.2]" aria-hidden="true">
          <path d="M12 4v11m0 0-4.5-4.5M12 15l4.5-4.5M5 20h14" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
        Download Envbyte
      </a>
      <p className="text-sm text-muted">For Windows 10 and 11. No admin rights needed.</p>
    </div>
  );
}
