import { RefreshCw } from "lucide-react";
import {
  api,
  type DetectionSource,
  type DetectionStatus,
  type Finding,
  type InstallSelection,
  type ProbeOutcome,
  type RobloxDetection,
  type RobloxInstall,
} from "../../api";
import { Button } from "../../components/ui/Button";
import { Card } from "../../components/ui/Card";
import { StatusPill, type Tone } from "../../components/ui/StatusPill";
import { useBackend } from "../../hooks/useBackend";
import { formatClock, formatDateTime } from "../../lib/format";
import { InstallPreferencePanel } from "./InstallPreferencePanel";

const STATUS: Record<DetectionStatus, { label: string; tone: Tone; explanation: string }> = {
  detected: {
    label: "Roblox files found",
    tone: "ok",
    explanation:
      "Roblox program files were found at an expected location. Checked: expected path, regular file, size above zero. Whether they are genuine Roblox software was not checked. Nexora did not start Roblox.",
  },
  notDetected: {
    label: "Not detected",
    tone: "neutral",
    explanation:
      "No supported Roblox installation location could be verified. Some installation types, including Microsoft Store installations, are not checked.",
  },
  inconclusive: {
    label: "Could not tell",
    tone: "warn",
    explanation:
      "Some places could not be checked, so Roblox may still be installed. This is not the same as not installed.",
  },
  unsupported: {
    label: "Not supported here",
    tone: "neutral",
    explanation: "Roblox detection only works on Windows.",
  },
};

const SOURCE_LABEL: Record<DetectionSource, string> = {
  versionFolder: "Roblox version folders",
  protocolHandler: "Windows link handler",
  microsoftStore: "Microsoft Store",
  platform: "This system",
};

const OUTCOME: Record<ProbeOutcome, { label: string; tone: Tone }> = {
  found: { label: "Passed basic checks", tone: "ok" },
  notFound: { label: "Not found", tone: "neutral" },
  missingExecutable: { label: "Program missing", tone: "warn" },
  notAFile: { label: "Empty or not a file", tone: "warn" },
  inaccessible: { label: "Could not check", tone: "warn" },
  incomplete: { label: "Partly checked", tone: "warn" },
  malformed: { label: "Ignored", tone: "warn" },
  unrecognised: { label: "Different program", tone: "warn" },
  unsupported: { label: "Not checked", tone: "neutral" },
  several: { label: "Several problems", tone: "warn" },
};

function InstallItem({ install, primary }: { install: RobloxInstall; primary: boolean }) {
  return (
    <li className="rounded-md border border-line p-3">
      <div className="mb-2 flex items-center gap-2">
        <span className="text-sm font-medium">{SOURCE_LABEL[install.source]}</span>
        {primary && <StatusPill tone="ok">Primary</StatusPill>}
      </div>
      <dl className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1.5 text-sm">
        <dt className="text-muted">Program</dt>
        <dd className="break-all">{install.executablePath}</dd>
        {install.version && (
          <>
            <dt className="text-muted">Version folder</dt>
            <dd className="break-all">{install.version}</dd>
          </>
        )}
        {install.modifiedAt !== null && (
          <>
            <dt className="text-muted">Last updated</dt>
            <dd>{formatDateTime(install.modifiedAt)}</dd>
          </>
        )}
      </dl>
    </li>
  );
}

function FindingItem({ finding }: { finding: Finding }) {
  const outcome = OUTCOME[finding.outcome];
  return (
    <li className="flex flex-wrap items-start gap-x-3 gap-y-1 text-sm">
      <span className="w-48 shrink-0 text-muted">{SOURCE_LABEL[finding.source]}</span>
      <StatusPill tone={outcome.tone}>{outcome.label}</StatusPill>
      <span className="min-w-0 flex-1 break-words">{finding.detail}</span>
    </li>
  );
}

/** The one-line answer, shown on screen. Its spoken form is the card's status sentence. */
function Headline({ detection, checkedAt }: { detection: RobloxDetection; checkedAt: number }) {
  const status = STATUS[detection.status];
  return (
    <div className="mb-1 flex items-center gap-3">
      <StatusPill tone={status.tone}>{status.label}</StatusPill>
      <span className="text-xs text-muted">Checked at {formatClock(checkedAt)}</span>
    </div>
  );
}

function Result({ selection, onSaved }: { selection: InstallSelection; onSaved: () => void }) {
  const { detection } = selection;
  const status = STATUS[detection.status];
  return (
    <>
      <p className="mb-4 text-sm text-muted">{status.explanation}</p>

      {detection.installs.length > 0 && (
        <ul className="mb-4 space-y-2" aria-label="Roblox installs found">
          {detection.installs.map((install, i) => (
            <InstallItem key={install.executablePath} install={install} primary={i === 0} />
          ))}
        </ul>
      )}

      <InstallPreferencePanel selection={selection} onSaved={onSaved} />

      <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted">What was checked</h3>
      <ul className="space-y-2" aria-label="What was checked">
        {detection.findings.map((finding, i) => (
          <FindingItem key={`${finding.source}-${i}`} finding={finding} />
        ))}
      </ul>
    </>
  );
}

/**
 * Whether Roblox is installed on this computer, and which installation Nexora prefers. Roblox
 * detection is read-only: it does not start Roblox or persist detection results. The card saves a
 * preferred installation source only when the user explicitly presses Save choice.
 */
export function RobloxStatusCard() {
  const { state, reload, refreshing } = useBackend(api.getInstallSelection);
  // A check is already running (first load or "Check again"): do not start another.
  const busy = refreshing || state.status === "loading";
  // What a screen reader says, as one whole sentence in one text node. It differs from the visible
  // wording on purpose (punctuation or phrasing), so the two are never mistaken for each other.
  // It also changes while a check runs, so a finished check is announced even when it ends in the
  // same second as the last one. A failed read is announced by its own alert below.
  const announcement =
    state.status === "loading"
      ? "Looking for Roblox."
      : state.status === "offline"
        ? "Roblox cannot be checked outside the Nexora window."
        : state.status === "ready"
          ? refreshing
            ? "Checking for Roblox again."
            : `Roblox check finished at ${formatClock(state.at)}: ${STATUS[state.data.detection.status].label}.`
          : "";
  return (
    <Card
      title="Roblox installation"
      actions={
        // While a check runs the button stays focusable (aria-disabled, not disabled), so keyboard
        // focus is not dropped; a press is ignored. Offline there is nothing to check, so it is
        // genuinely disabled.
        <Button
          onClick={busy ? undefined : reload}
          disabled={state.status === "offline"}
          aria-disabled={busy || undefined}
          className={busy ? "cursor-not-allowed opacity-50" : ""}
        >
          <RefreshCw aria-hidden="true" className={`h-4 w-4 ${refreshing ? "animate-spin" : ""}`} />
          {refreshing ? "Checking…" : "Check again"}
        </Button>
      }
    >
      {/* Always mounted, so screen readers announce it whenever the sentence changes. Only this
          and the installation panel's own region are live; the text below is not. */}
      <p role="status" aria-live="polite" aria-atomic="true" className="sr-only">
        {announcement}
      </p>
      <div>
        {state.status === "loading" && <p className="text-sm text-muted">Looking for Roblox…</p>}
        {state.status === "offline" && (
          <p className="text-sm text-muted">Roblox detection only works inside the Nexora window.</p>
        )}
        {state.status === "ready" && <Headline detection={state.data.detection} checkedAt={state.at} />}
        {refreshing && state.status !== "loading" && (
          <p className="text-xs text-muted">Checking for Roblox again…</p>
        )}
      </div>
      {state.status === "error" && (
        <p role="alert" className="text-sm text-danger">
          {state.message}
        </p>
      )}
      {state.status === "ready" && <Result selection={state.data} onSaved={reload} />}
    </Card>
  );
}
