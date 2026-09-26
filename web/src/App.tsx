import { CallToAction } from "./components/CallToAction";
import { Commands } from "./components/Commands";
import { Hero } from "./components/Hero";
import { HowItWorks } from "./components/HowItWorks";
import { Page } from "./components/Page";
import { Quickstart } from "./components/Quickstart";
import { SelfHost } from "./components/SelfHost";

export function App() {
  return (
    <Page current="home">
      <Hero />
      <HowItWorks />
      <Commands />
      <Quickstart />
      <SelfHost />
      <CallToAction />
    </Page>
  );
}
