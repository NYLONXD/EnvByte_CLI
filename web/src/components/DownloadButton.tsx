import { useEffect, useState } from "react";
import { MACOS_PKG_URL, WINDOWS_MSI_URL } from "../lib/content";
import { detectPlatform, type Platform } from "../lib/platform";

const INSTALLERS = {
  mac: {
    url: MACOS_PKG_URL,
    label: "Download for macOS",
    note: "Apple silicon and Intel.",
    other: "Mac installer",
  },
  windows: {
    url: WINDOWS_MSI_URL,
    label: "Download for Windows",
    note: "Windows 10 and 11. No admin rights needed.",
    other: "Windows installer",
  },
};

const LINK = "underline decoration-plain/35 underline-offset-4 hover:text-fg hover:decoration-plain";

/**
 * The installer for the visitor's system: envbyte.pkg on a Mac, which puts the
 * CLI in /usr/local/bin, or envbyte.msi on Windows, which installs it for the
 * current user and adds it to PATH. The other one stays a link away, for
 * downloading on one machine to install on another. Linux has no installer
 * file; the command above is the way in there.
 */
export function DownloadButton() {
  const [platform, setPlatform] = useState<Platform>("other");
  useEffect(() => setPlatform(detectPlatform()), []);

  if (platform === "linux") {
    return (
      <p className="text-sm text-muted">
        On Linux, the command above installs Envbyte. There are installers for{" "}
        <a href={MACOS_PKG_URL} className={LINK}>
          macOS
        </a>{" "}
        and{" "}
        <a href={WINDOWS_MSI_URL} className={LINK}>
          Windows
        </a>
        .
      </p>
    );
  }

  const primary = platform === "windows" ? INSTALLERS.windows : INSTALLERS.mac;
  const secondary = platform === "windows" ? INSTALLERS.mac : INSTALLERS.windows;
  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
      <a href={primary.url} className="button">
        <svg viewBox="0 0 24 24" className="size-4.5 fill-none stroke-current stroke-[2.2]" aria-hidden="true">
          <path d="M12 4v11m0 0-4.5-4.5M12 15l4.5-4.5M5 20h14" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
        {primary.label}
      </a>
      <p className="text-sm text-muted">
        {primary.note}{" "}
        <a href={secondary.url} className={LINK}>
          {secondary.other}
        </a>
      </p>
    </div>
  );
}
