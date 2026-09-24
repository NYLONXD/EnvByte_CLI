import { CallToAction } from "./components/CallToAction";
import { Commands } from "./components/Commands";
import { Footer } from "./components/Footer";
import { Header } from "./components/Header";
import { Hero } from "./components/Hero";
import { HowItWorks } from "./components/HowItWorks";
import { Quickstart } from "./components/Quickstart";
import { SelfHost } from "./components/SelfHost";

export function App() {
  return (
    <>
      <a
        href="#main"
        className="absolute -top-12 left-4 z-30 rounded-full bg-mint px-4 py-2 font-semibold text-on-mint focus:top-3"
      >
        Skip to content
      </a>
      <Header />
      <main id="main">
        <Hero />
        <HowItWorks />
        <Commands />
        <Quickstart />
        <SelfHost />
        <CallToAction />
      </main>
      <Footer />
    </>
  );
}
