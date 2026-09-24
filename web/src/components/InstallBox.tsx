import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { INSTALL_METHODS } from "../lib/content";

/** Picks the tab a visitor most likely wants, from what the browser reports. */
function detectDefaultMethod(): string {
  if (typeof navigator === "undefined") return "sh";
  const platform =
    (navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData?.platform ??
    navigator.userAgent;
  return /win/i.test(platform) ? "powershell" : "sh";
}

async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

export function InstallBox() {
  const [selectedId, setSelectedId] = useState("sh");
  const [copied, setCopied] = useState(false);
  const tabRefs = useRef<Record<string, HTMLButtonElement | null>>({});
  const resetTimer = useRef<number | undefined>(undefined);

  useEffect(() => {
    setSelectedId(detectDefaultMethod());
    return () => window.clearTimeout(resetTimer.current);
  }, []);

  const selected = INSTALL_METHODS.find((method) => method.id === selectedId) ?? INSTALL_METHODS[0];

  async function handleCopy() {
    if (!(await copyText(selected.command))) return;
    setCopied(true);
    window.clearTimeout(resetTimer.current);
    resetTimer.current = window.setTimeout(() => setCopied(false), 1800);
  }

  // Arrow keys move between tabs, as the WAI-ARIA tabs pattern expects.
  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const index = INSTALL_METHODS.findIndex((method) => method.id === selectedId);
    const step = event.key === "ArrowRight" ? 1 : event.key === "ArrowLeft" ? -1 : 0;
    if (!step) return;
    event.preventDefault();
    const next = INSTALL_METHODS[(index + step + INSTALL_METHODS.length) % INSTALL_METHODS.length];
    setSelectedId(next.id);
    setCopied(false);
    tabRefs.current[next.id]?.focus();
  }

  return (
    <div id="install" className="max-w-155 scroll-mt-24" data-animate="install">
      <div
        role="tablist"
        aria-label="Install method"
        onKeyDown={handleKeyDown}
        className="flex gap-1 overflow-x-auto rounded-t-xl border border-b-0 border-line bg-raised p-1 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
      >
        {INSTALL_METHODS.map((method) => {
          const active = method.id === selected.id;
          return (
            <button
              key={method.id}
              ref={(element) => {
                tabRefs.current[method.id] = element;
              }}
              type="button"
              role="tab"
              id={`install-tab-${method.id}`}
              aria-selected={active}
              aria-controls="install-panel"
              tabIndex={active ? 0 : -1}
              onClick={() => {
                setSelectedId(method.id);
                setCopied(false);
              }}
              className={`flex-none rounded-lg px-2.5 py-1.5 text-[0.8rem] transition-colors ${
                active ? "bg-accent-soft font-semibold text-accent" : "text-muted hover:text-fg"
              }`}
            >
              {method.label}
            </button>
          );
        })}
      </div>

      <div
        id="install-panel"
        role="tabpanel"
        aria-labelledby={`install-tab-${selected.id}`}
        className="flex items-center gap-3 rounded-b-xl border border-line bg-term py-3.5 pr-3.5 pl-4.5"
      >
        <pre className="min-w-0 flex-1 text-[0.82rem] leading-7 sm:text-[0.9rem] whitespace-pre-wrap text-term-fg [overflow-wrap:anywhere]">
          <code>
            <span className="text-mint select-none">{selected.shell === "powershell" ? "> " : "$ "}</span>
            {selected.command}
          </code>
        </pre>
        <button
          type="button"
          onClick={handleCopy}
          aria-label="Copy install command"
          className={`inline-flex flex-none items-center gap-1.5 rounded-lg border px-2.5 py-2 sm:px-3 sm:py-1.5 text-sm transition-colors ${
            copied ? "border-mint text-mint" : "border-white/15 text-term-fg hover:border-mint"
          }`}
        >
          <svg viewBox="0 0 24 24" className="size-3.75 fill-none stroke-current stroke-2" aria-hidden="true">
            {copied ? (
              <path d="m5 12 5 5 9-10" strokeLinecap="round" strokeLinejoin="round" />
            ) : (
              <>
                <rect x="9" y="9" width="11" height="11" rx="2" />
                <path d="M5 15V5a2 2 0 0 1 2-2h10" strokeLinecap="round" />
              </>
            )}
          </svg>
          <span className="hidden sm:inline">{copied ? "Copied" : "Copy"}</span>
        </button>
      </div>

      <p className="mt-3 min-h-6 text-sm text-muted" aria-live="polite">
        {copied ? "Copied to your clipboard." : selected.note}
      </p>
    </div>
  );
}
