import type { ReactNode } from "react";
import { Footer } from "./Footer";
import { Header, type PageId } from "./Header";

/** The frame every page shares. The header floats, so each page's first
 *  section leaves room for it at the top. */
export function Page({ current, children }: { current: PageId; children: ReactNode }) {
  return (
    <>
      <a
        href="#main"
        className="absolute -top-12 left-4 z-30 rounded-full bg-mint px-4 py-2 font-semibold text-on-mint focus:top-3"
      >
        Skip to content
      </a>
      <Header current={current} />
      <main id="main">{children}</main>
      <Footer />
    </>
  );
}
