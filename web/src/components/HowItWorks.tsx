import { useRef } from "react";
import { MOTION_OK, ScrollTrigger, gsap, useGSAP } from "../lib/gsap";

const STEPS = [
  {
    title: "Encrypt locally",
    body: (
      <>
        <code className="chip">envbyte push</code> encrypts your file with AES-256-GCM on your own machine. Only
        ciphertext is uploaded.
      </>
    ),
  },
  {
    title: "A sealed key for each person",
    body: "The project key is sealed to each member's personal X25519 identity key. Inviting someone seals a copy for them, so there's nothing to paste into chat.",
  },
  {
    title: "The server stores noise",
    body: "The server keeps ciphertext and sealed keys it cannot open. A leaked database would give an attacker nothing to read.",
  },
];

const NODES = [
  { x: 4, width: 236, title: "Your laptop", mono: ".env → AES-256-GCM", muted: "plaintext never leaves" },
  {
    x: 364,
    width: 236,
    title: "Envbyte server",
    mono: "ciphertext + sealed keys",
    muted: "holds no key to open them",
    accent: true,
  },
  { x: 724, width: 172, title: "Teammate", mono: "AES-256-GCM → .env", muted: "opens their own copy" },
];

const LINKS = [
  { from: 248, to: 356, label: "ciphertext" },
  { from: 608, to: 716, label: "their sealed key" },
];

/** Laptop → server → teammate, with packets travelling along the arrows. */
function FlowDiagram() {
  return (
    <svg
      viewBox="0 0 900 180"
      className="mt-12 mb-2 hidden w-full sm:block"
      role="img"
      aria-labelledby="flow-title flow-desc"
    >
      <title id="flow-title">How a .env file travels through Envbyte</title>
      <desc id="flow-desc">
        Your laptop encrypts the file and uploads only ciphertext. The server stores ciphertext and one sealed copy of
        the project key per member. A teammate's laptop opens its own sealed copy and decrypts the file.
      </desc>
      <defs>
        <marker id="flow-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto">
          <path d="M0 0 10 5 0 10z" className="fill-muted" />
        </marker>
      </defs>

      {NODES.map((node) => (
        <g key={node.title} data-flow-node>
          <rect
            x={node.x}
            y="30"
            width={node.width}
            height="120"
            rx="16"
            className={node.accent ? "fill-accent-soft stroke-mint" : "fill-raised stroke-line-strong"}
          />
          <text
            x={node.x + node.width / 2}
            y="74"
            textAnchor="middle"
            className="fill-fg font-display text-[19px] font-bold"
          >
            {node.title}
          </text>
          <text x={node.x + node.width / 2} y="102" textAnchor="middle" className="fill-accent font-mono text-[13px]">
            {node.mono}
          </text>
          <text x={node.x + node.width / 2} y="126" textAnchor="middle" className="fill-muted text-[13px]">
            {node.muted}
          </text>
        </g>
      ))}

      {LINKS.map((link) => (
        <g key={link.label}>
          <path
            d={`M${link.from} 90H${link.to}`}
            data-flow-line
            className="fill-none stroke-muted"
            strokeWidth="1.5"
            strokeDasharray="5 5"
            markerEnd="url(#flow-arrow)"
          />
          <text x={(link.from + link.to) / 2} y="78" textAnchor="middle" className="fill-muted text-[13px]">
            {link.label}
          </text>
          <circle cx={link.from} cy="90" r="4.5" className="fill-mint opacity-0" data-flow-packet />
        </g>
      ))}
    </svg>
  );
}

export function HowItWorks() {
  const scope = useRef<HTMLElement>(null);

  useGSAP(
    () => {
      const mm = gsap.matchMedia();
      mm.add(MOTION_OK, () => {
        gsap.from("[data-flow-node]", {
          autoAlpha: 0,
          y: 18,
          duration: 0.7,
          stagger: 0.18,
          ease: "power3.out",
          scrollTrigger: { trigger: "[data-flow-node]", start: "top 85%", once: true },
        });

        // The dashes march in the direction data flows.
        gsap.to("[data-flow-line]", { strokeDashoffset: -20, duration: 1, ease: "none", repeat: -1 });

        // One packet crosses to the server, then the next leaves for the
        // teammate, on a loop that only runs while the diagram is on screen.
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
    <section ref={scope} id="how" className="scroll-mt-12 py-16 sm:py-24 lg:py-28">
      <div className="container-page">
        <p className="kicker" data-reveal>
          How it works
        </p>
        <h2 className="max-w-[22em] text-[clamp(1.8rem,3.6vw,2.6rem)] leading-tight font-bold" data-reveal>
          Encrypted before it leaves. Unlockable only by your team.
        </h2>

        <FlowDiagram />

        <ol className="mt-10 grid gap-5 md:grid-cols-3">
          {STEPS.map((step, index) => (
            <li key={step.title} className="card" data-reveal>
              <span className="mb-4 inline-grid size-8 place-items-center rounded-lg bg-accent-soft font-mono font-semibold text-accent">
                {index + 1}
              </span>
              <h3 className="text-lg font-bold">{step.title}</h3>
              <p className="mt-2.5 text-muted">{step.body}</p>
            </li>
          ))}
        </ol>
      </div>
    </section>
  );
}
