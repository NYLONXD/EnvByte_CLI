import { useRef, type ReactNode } from "react";
import { MOTION_OK, ScrollTrigger, gsap, useGSAP } from "../lib/gsap";

interface Stop {
  title: string;
  /** Mint where the file can be read, copper where it is sealed. */
  sealed: boolean;
  diagram: { name: string; detail: string; note: string };
  body: ReactNode;
}

// Each step sits directly under the part of the diagram it describes.
const STOPS: Stop[] = [
  {
    title: "Encrypted on your laptop",
    sealed: false,
    diagram: { name: "Your laptop", detail: ".env → AES-256-GCM", note: "plaintext stays here" },
    body: (
      <>
        <code className="chip">envbyte push</code> encrypts the whole file before it leaves your machine, variable
        names included.
      </>
    ),
  },
  {
    title: "Stored as noise",
    sealed: true,
    diagram: { name: "Envbyte server", detail: "ciphertext + sealed keys", note: "holds no key to open them" },
    body: "The server keeps the ciphertext and one sealed copy of the project key per member. A leaked database gives an attacker nothing to read.",
  },
  {
    title: "Opened with their own key",
    sealed: false,
    diagram: { name: "Teammate", detail: "sealed key → .env", note: "opens their own copy" },
    body: "Each copy of the project key is sealed to one person's X25519 identity. Inviting someone seals a copy for them, so nothing gets pasted into chat.",
  },
];

// Three equal nodes on a 900-wide canvas, 120 apart; the step columns below
// use the same proportions so they line up with them.
const NODE_WIDTH = 220;
const NODE_GAP = 120;
const nodeX = (index: number) => index * (NODE_WIDTH + NODE_GAP);
const LINKS = [
  { from: nodeX(0) + NODE_WIDTH + 8, to: nodeX(1) - 8, label: "ciphertext" },
  { from: nodeX(1) + NODE_WIDTH + 8, to: nodeX(2) - 8, label: "sealed key" },
];

function FlowDiagram() {
  return (
    <svg viewBox="0 0 900 150" className="hidden w-full overflow-visible sm:block" role="img" aria-labelledby="flow-title flow-desc">
      <title id="flow-title">How a .env file travels through Envbyte</title>
      <desc id="flow-desc">
        Your laptop encrypts the file and uploads only ciphertext. The server stores ciphertext and one sealed copy of
        the project key per member. A teammate's laptop opens its own sealed copy and decrypts the file.
      </desc>
      <defs>
        <marker id="flow-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto">
          <path d="M0 0 10 5 0 10z" className="fill-sealed" />
        </marker>
      </defs>

      {STOPS.map((stop, index) => {
        const x = nodeX(index);
        const middle = x + NODE_WIDTH / 2;
        return (
          <g key={stop.title}>
            <rect
              x={x + 0.75}
              y="15"
              width={NODE_WIDTH - 1.5}
              height="120"
              rx="14"
              strokeWidth="1.5"
              className={stop.sealed ? "fill-sealed-soft stroke-sealed/55" : "fill-plain-soft stroke-plain/45"}
            />
            <text x={middle} y="60" textAnchor="middle" className="fill-fg font-sans text-[18px] font-semibold">
              {stop.diagram.name}
            </text>
            <text
              x={middle}
              y="88"
              textAnchor="middle"
              className={`font-mono text-[12px] [font-stretch:87.5%] ${stop.sealed ? "fill-sealed" : "fill-plain"}`}
            >
              {stop.diagram.detail}
            </text>
            <text x={middle} y="112" textAnchor="middle" className="fill-muted font-sans text-[13px]">
              {stop.diagram.note}
            </text>
          </g>
        );
      })}

      {LINKS.map((link) => (
        <g key={link.label}>
          <path
            d={`M${link.from} 75H${link.to}`}
            data-flow-line
            className="fill-none stroke-sealed/70"
            strokeWidth="1.5"
            strokeDasharray="5 5"
            markerEnd="url(#flow-arrow)"
          />
          <text x={(link.from + link.to) / 2} y="62" textAnchor="middle" className="fill-muted font-sans text-[12.5px]">
            {link.label}
          </text>
          <circle cx={link.from} cy="75" r="4.5" className="fill-sealed opacity-0" data-flow-packet />
        </g>
      ))}
    </svg>
  );
}

export function HowItWorks() {
  const scope = useRef<HTMLElement>(null);

  // Motion that explains: the dashes march the way data flows, and a packet
  // of ciphertext crosses each link, only while the diagram is on screen.
  useGSAP(
    () => {
      const mm = gsap.matchMedia();
      mm.add(MOTION_OK, () => {
        gsap.to("[data-flow-line]", { strokeDashoffset: -20, duration: 1, ease: "none", repeat: -1 });

        const packets = gsap.utils.toArray<SVGCircleElement>("[data-flow-packet]");
        const loop = gsap.timeline({ repeat: -1, repeatDelay: 0.6, paused: true });
        packets.forEach((packet, index) => {
          const link = LINKS[index];
          loop
            .set(packet, { attr: { cx: link.from }, opacity: 1 })
            .to(packet, { attr: { cx: link.to - 8 }, duration: 1.1, ease: "power1.inOut" })
            .to(packet, { opacity: 0, duration: 0.2 });
        });
        ScrollTrigger.create({
          trigger: scope.current,
          start: "top bottom",
          end: "bottom top",
          onToggle: (self) => (self.isActive ? loop.play() : loop.pause()),
        });
      });
    },
    { scope },
  );

  return (
    <section ref={scope} id="how" className="scroll-mt-12 border-t border-line py-20 sm:py-24 lg:py-32">
      <div className="container-page">
        <h2 className="heading-2 max-w-[29ch]">Encrypted before it leaves. Readable only by your team.</h2>

        <div className="mt-12 lg:mt-16">
          <FlowDiagram />
          <ol className="mt-10 grid max-w-[40rem] gap-9 lg:max-w-none lg:grid-cols-3 lg:gap-x-[13.3333%]">
            {STOPS.map((stop) => (
              <li key={stop.title} className={`border-t-2 pt-5 ${stop.sealed ? "border-sealed" : "border-plain"}`}>
                <h3 className="text-[1.125rem]">{stop.title}</h3>
                <p className="mt-2 text-muted">{stop.body}</p>
              </li>
            ))}
          </ol>
        </div>
      </div>
    </section>
  );
}
