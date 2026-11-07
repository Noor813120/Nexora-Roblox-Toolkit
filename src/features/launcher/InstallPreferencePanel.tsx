import { useId, useState } from "react";
import {
  api,
  type InstallChoice,
  type InstallSelection,
  type PreferenceReason,
  type PreferenceResolution,
  type PreferredSource,
} from "../../api";
import { Button } from "../../components/ui/Button";
import { StatusPill, type Tone } from "../../components/ui/StatusPill";
import { useAction } from "../../hooks/useAction";

const CHOICES: InstallChoice[] = ["automatic", "protocolHandler", "versionFolder"];

const SOURCE_NAME: Record<PreferredSource, string> = {
  protocolHandler: "Windows link handler",
  versionFolder: "Roblox version folders",
};

const CHOICE: Record<InstallChoice, { label: string; help: string }> = {
  automatic: { label: "Automatic", help: "Nexora uses the first installation it finds." },
  protocolHandler: {
    label: SOURCE_NAME.protocolHandler,
    help: "The installation Windows opens for Roblox links.",
  },
  versionFolder: {
    label: SOURCE_NAME.versionFolder,
    help: "The newest installation in Roblox's version folders.",
  },
};

const UNSUPPORTED: Record<PreferenceReason, string> = {
  newerFormat: "Your saved choice was written by a newer version of Nexora, so it was left unchanged.",
  unsupportedSource: "Your saved choice names a source this version cannot use, so it was left unchanged.",
  unsupportedPlatform: "Your saved choice can only be used on Windows.",
};

interface Status {
  label: string;
  tone: Tone;
  text: string;
  /** The program that would be used. Runtime information only: it is shown, never saved. */
  program: string | null;
}

/** What the saved preference means right now, in words. Never repeats any stored text. */
function describe(p: PreferenceResolution): Status {
  const saved = p.preferred ? `Your saved choice, ${SOURCE_NAME[p.preferred]},` : "Your saved choice";
  const program = p.chosen?.executablePath ?? null;
  switch (p.state) {
    case "automatic":
      return {
        label: "Automatic",
        tone: "neutral",
        text: program ? "Using the first installation found." : "No installation was found to use.",
        program,
      };
    case "available": {
      const n = p.candidateCount;
      const name = p.preferred ? SOURCE_NAME[p.preferred] : "Saved source";
      return {
        label: "Available",
        tone: "ok",
        text: `${name}: using 1 of ${n} ${n === 1 ? "installation" : "installations"} found.`,
        program,
      };
    }
    case "unavailable":
      return {
        label: "Not available",
        tone: "warn",
        text: `${saved} was not found this time. Nothing else was used in its place.`,
        program: null,
      };
    case "inconclusive": {
      const target = p.preferred ? `the saved ${SOURCE_NAME[p.preferred]}` : "your saved choice";
      return {
        label: "Could not tell",
        tone: "warn",
        text: `Some places could not be checked, so ${target} may still exist. Nothing else was used in its place.`,
        program: null,
      };
    }
    case "unsupported":
      return {
        label: "Not supported here",
        tone: "neutral",
        text: p.reason ? UNSUPPORTED[p.reason] : "Your saved choice cannot be used here.",
        program: null,
      };
    case "invalid":
      return {
        label: "Unreadable",
        tone: "warn",
        text: "Your saved choice could not be read, so it was left unchanged. Pick an option and save to replace it.",
        program: null,
      };
  }
}

/**
 * The choice that is saved right now, or `null` when a value is saved but cannot be used (so no
 * option may look selected, and any pick, "Automatic" included, is a change).
 */
function currentChoice(p: PreferenceResolution): InstallChoice | null {
  switch (p.state) {
    case "automatic":
      return "automatic";
    case "invalid":
      return null;
    default:
      return p.preferred;
  }
}

/**
 * Which Roblox installation Nexora should prefer. It shows what the saved choice means right now
 * and lets the user change it. Nothing here starts Roblox, and choosing only saves a preference.
 *
 * - `selection.preference === null` means the choice cannot be read or saved: the whole control is
 *   disabled, and that is not shown as "Automatic".
 * - A saved source that is not available is shown as saved (selected but disabled); nothing else
 *   is selected in its place.
 * - Only sources in `selection.availableSources` can be picked, and "Automatic" always can.
 * - Saving is explicit. After a save the parent reloads the whole selection (`onSaved`), so the
 *   screen shows what the backend now says, not what was submitted.
 */
export function InstallPreferencePanel({
  selection,
  onSaved,
}: {
  selection: InstallSelection;
  onSaved: () => void;
}) {
  const { preference, availableSources } = selection;
  const groupName = useId();
  const current = preference ? currentChoice(preference) : null;
  const [draft, setDraft] = useState<InstallChoice | null>(current);
  // Follow the saved choice when it changes (after a save or a reload), and only then, so a pick
  // the user has not saved yet survives a "Check again". Done while rendering rather than in an
  // effect: an effect also runs at mount and could overwrite a pick made just before it ran.
  const [seen, setSeen] = useState(current);
  if (seen !== current) {
    setSeen(current);
    setDraft(current);
  }
  const save = useAction(api.setPreferredInstall);

  const usable = preference !== null;
  const status = preference ? describe(preference) : null;
  const dirty = usable && draft !== null && draft !== current;
  const canSave = dirty && !save.pending && save.state.status !== "done";
  const offered = (choice: InstallChoice) => choice === "automatic" || availableSources.includes(choice);

  const onSave = async () => {
    if (draft === null) return;
    if (await save.run(draft)) onSaved();
  };

  return (
    <div className="mb-4 border-t border-line pt-4">
      {/* Always mounted, so screen readers announce what changes inside it. */}
      <div aria-live="polite" className="mb-3">
        {status ? (
          <>
            <div className="flex flex-wrap items-center gap-3">
              <StatusPill tone={status.tone}>{status.label}</StatusPill>
              <span className="text-sm text-muted">{status.text}</span>
            </div>
            {status.program && <p className="mt-1 break-all text-sm">Program: {status.program}</p>}
          </>
        ) : (
          <div className="flex flex-wrap items-center gap-3">
            <StatusPill tone="warn">Preference unavailable</StatusPill>
            <span className="text-sm text-muted">
              Preference storage is unavailable, so your choice cannot be shown or saved right now.
            </span>
          </div>
        )}
      </div>

      <fieldset disabled={!usable || save.pending}>
        <legend className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted">
          Preferred installation
        </legend>
        <div className="space-y-2">
          {CHOICES.map((choice) => {
            const id = `${groupName}-${choice}`;
            const available = offered(choice);
            return (
              <div key={choice} className="flex items-start gap-2 text-sm">
                <input
                  id={id}
                  type="radio"
                  name={groupName}
                  value={choice}
                  checked={draft === choice}
                  disabled={!available}
                  aria-describedby={`${id}-help`}
                  onChange={() => {
                    setDraft(choice);
                    save.reset();
                  }}
                  className="mt-1"
                />
                <div>
                  <label htmlFor={id} className="font-medium">
                    {CHOICE[choice].label}
                    {!available && " — not available now"}
                  </label>
                  <p id={`${id}-help`} className="text-xs text-muted">
                    {CHOICE[choice].help}
                  </p>
                </div>
              </div>
            );
          })}
        </div>
      </fieldset>

      <div className="mt-3 flex flex-wrap items-center gap-3">
        <Button variant="primary" onClick={onSave} disabled={!canSave}>
          {save.pending ? "Saving…" : "Save choice"}
        </Button>
        {save.state.status === "done" && (
          <p role="status" className="text-xs text-ok">
            Saved.
          </p>
        )}
      </div>
      {save.state.status === "error" && (
        <p role="alert" className="mt-2 text-sm text-danger">
          {save.state.message}
        </p>
      )}
    </div>
  );
}
