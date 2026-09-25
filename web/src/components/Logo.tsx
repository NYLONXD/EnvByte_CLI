import { useId } from "react";

/**
 * The Envbyte mark: a padlock inside a hexagon, dark teal on the left and mint
 * on the right. This is the simplified cut that stays legible down to 16px;
 * public/logo.svg is the full one, with the cube edges and "eb" on the lock.
 */
export function LogoMark({ size = 30, className = "" }: { size?: number; className?: string }) {
  // Header and footer both render the mark, so its gradient and mask ids must
  // be unique on the page.
  const id = `logo${useId().replace(/[^\w-]/g, "")}`;
  return (
    <svg viewBox="0 0 64 64" width={size} height={size} className={className} aria-hidden="true">
      <defs>
        <linearGradient id={`${id}l`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#4DBBA2" />
          <stop offset="1" stopColor="#1B6A67" />
        </linearGradient>
        <linearGradient id={`${id}r`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#8FF2C9" />
          <stop offset="1" stopColor="#35C990" />
        </linearGradient>
        <mask id={`${id}m`} maskUnits="userSpaceOnUse" x="0" y="0" width="64" height="64">
          <g fill="#fff" stroke="#fff">
            <path
              d="M32 2L57.981 17V47L32 62L6.019 47V17ZM32 11.238L49.981 21.619V42.381L32 52.762L14.019 42.381V21.619Z"
              fillRule="evenodd"
              stroke="none"
            />
            <path d="M26.8 28V23A5.2 5.2 0 0 1 37.2 23V28" fill="none" strokeWidth="4.2" />
            <path d="M22.4 27.5H41.6Q44 27.5 44 29.9V41.5L32 48.5L20 41.5V29.9Q20 27.5 22.4 27.5Z" stroke="none" />
          </g>
        </mask>
      </defs>
      <g mask={`url(#${id}m)`}>
        <rect width="32" height="64" fill={`url(#${id}l)`} />
        <rect x="32" width="32" height="64" fill={`url(#${id}r)`} />
      </g>
    </svg>
  );
}

export function Wordmark({ size = 30, className = "" }: { size?: number; className?: string }) {
  return (
    <span
      className={`inline-flex items-center gap-2.5 font-mono font-semibold tracking-[-0.04em] [font-stretch:112.5%] ${className}`}
    >
      <LogoMark size={size} />
      envbyte
    </span>
  );
}
