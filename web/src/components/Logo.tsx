/** The Envbyte mark: a keyhole whose head is the dot of `.env`. */
export function LogoMark({ size = 30, className = "" }: { size?: number; className?: string }) {
  return (
    <svg
      viewBox="0 0 64 64"
      width={size}
      height={size}
      className={className}
      aria-hidden="true"
    >
      <rect width="64" height="64" rx="15" fill="#0B1020" />
      <circle cx="32" cy="25.5" r="9.5" fill="#3EE3A3" />
      <path
        d="M27.6 30.5h8.8l3 14.7c.3 1.6-.9 2.8-2.5 2.8h-9.8c-1.6 0-2.8-1.2-2.5-2.8z"
        fill="#3EE3A3"
      />
    </svg>
  );
}

export function Wordmark({ size = 30, className = "" }: { size?: number; className?: string }) {
  return (
    <span className={`inline-flex items-center gap-2.5 font-display font-bold tracking-tight ${className}`}>
      <LogoMark size={size} />
      envbyte
    </span>
  );
}
