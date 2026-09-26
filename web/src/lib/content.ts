// Copy and links that appear in more than one component.

export const SITE_URL = "https://envbyte.trackedge.in";
export const REPO_URL = "https://github.com/NYLONXD/EnvByte_CLI";
export const API_STATUS_URL = "https://api.envbyte.trackedge.in/health/ready";
export const AUTHOR_GITHUB_URL = "https://github.com/NYLONXD";
/** The Windows installer (packaging/msi in the repo), attached to every release. */
export const WINDOWS_MSI_URL = `${REPO_URL}/releases/latest/download/envbyte.msi`;

export type Shell = "sh" | "powershell";

export interface InstallMethod {
  id: string;
  label: string;
  command: string;
  shell: Shell;
  note: string;
}

export const INSTALL_METHODS: InstallMethod[] = [
  {
    id: "sh",
    label: "macOS / Linux",
    command: `curl -fsSL ${SITE_URL}/install.sh | sh`,
    shell: "sh",
    note: "Installs to ~/.envbyte/bin and checks the download's SHA-256 first.",
  },
  {
    id: "msi",
    label: "Windows",
    // msiexec downloads the installer itself and follows GitHub's redirects.
    command: `msiexec /i ${WINDOWS_MSI_URL}`,
    shell: "powershell",
    note: "Opens the Envbyte installer. Per-user, so no administrator rights needed.",
  },
  {
    id: "powershell",
    label: "PowerShell",
    // Wrapped in `powershell -c` so it runs the same from Command Prompt,
    // PowerShell or Windows Terminal; bare `irm` exists only in PowerShell.
    command: `powershell -c "irm ${SITE_URL}/install.ps1 | iex"`,
    shell: "powershell",
    note: "Installs to ~\\.envbyte\\bin without an installer window. No administrator rights needed.",
  },
  {
    id: "npm",
    label: "npm",
    command: "npm install -g envbyte",
    shell: "sh",
    note: "Pulls in the prebuilt binary for your platform.",
  },
  {
    id: "brew",
    label: "Homebrew",
    command: "brew install nylonxd/tap/envbyte",
    shell: "sh",
    note: "macOS and Linux.",
  },
  {
    id: "scoop",
    label: "Scoop",
    command: "scoop bucket add nylonxd https://github.com/NYLONXD/scoop-bucket; scoop install nylonxd/envbyte",
    shell: "powershell",
    note: "Windows, per-user, no administrator rights needed.",
  },
  {
    id: "winget",
    label: "winget",
    command: "winget install Envbyte.Envbyte",
    shell: "powershell",
    note: "Windows Package Manager.",
  },
  {
    id: "cargo",
    label: "Cargo",
    command: "cargo install envbyte",
    shell: "sh",
    note: "Builds from source. Needs Rust 1.88 or newer.",
  },
];

export type TerminalLine =
  | { kind: "command"; text: string }
  | { kind: "output" | "success" | "comment"; text: string };

/** The hero's example session. */
export const TERMINAL_SCRIPT: TerminalLine[] = [
  { kind: "command", text: "envbyte create payments-api" },
  { kind: "success", text: "✓ Created payments-api" },
  { kind: "command", text: 'envbyte push -m "add stripe keys"' },
  { kind: "output", text: "  encrypting .env on this machine" },
  { kind: "success", text: "✓ Pushed version 3. The server can't read it" },
  { kind: "command", text: "envbyte add priya@company.com" },
  { kind: "success", text: "✓ Invited priya@company.com" },
  { kind: "comment", text: "# meanwhile, on Priya's laptop" },
  { kind: "command", text: "envbyte init && envbyte pull" },
  { kind: "success", text: "✓ Decrypted .env · 14 variables" },
];

/**
 * The file in the hero's lens. Made-up values, and none in a format a
 * provider's secret scanner looks for, so pushes are not blocked over them.
 */
export const ENV_SAMPLE = [
  "# payments-api, production",
  "DATABASE_URL=postgres://app:Hx7pQ2vL@db.internal/pay",
  "STRIPE_API_KEY=demo_51NfK2bL9xQ7mZ4cR8vYw",
  "SESSION_SECRET=7c1e9f04b2d86a35e1f0c4",
  "REDIS_URL=redis://:a8Kd2mQ@cache.internal:6379",
  "MAILER_TOKEN=mt_8f2c1e9d04b7a653",
  "CHECKOUT_V2=true",
];
