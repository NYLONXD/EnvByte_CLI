export type Platform = "windows" | "mac" | "linux" | "other";

/**
 * The visitor's operating system, from what the browser reports. Runs only in
 * the browser: the prerendered page has no navigator, and says "other".
 */
export function detectPlatform(): Platform {
  if (typeof navigator === "undefined") return "other";
  const hint = (navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData?.platform ?? "";
  const agent = navigator.userAgent;
  if (/android|iphone|ipad|ipod/i.test(`${hint} ${agent}`)) return "other";
  if (/windows/i.test(`${hint} ${agent}`)) return "windows";
  if (/mac/i.test(`${hint} ${agent}`)) {
    // iPadOS asks for desktop sites with a Mac user agent; only touch gives
    // it away.
    return navigator.maxTouchPoints > 1 ? "other" : "mac";
  }
  if (/linux|cros|x11/i.test(`${hint} ${agent}`)) return "linux";
  return "other";
}
