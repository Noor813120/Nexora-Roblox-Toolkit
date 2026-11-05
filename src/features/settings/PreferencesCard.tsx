import { useEffect, useId, useState } from "react";
import { api, type SettingEntry } from "../../api";
import { Button } from "../../components/ui/Button";
import { Card } from "../../components/ui/Card";
import { useAction } from "../../hooks/useAction";
import { useBackend } from "../../hooks/useBackend";

/** Words for the settings the backend knows. A key without an entry here still works; it shows its key. */
const TEXT: Record<string, { label: string; help: string; unit: string }> = {
  "activity.retention_days": {
    label: "Keep activity history for",
    help: "Older entries are removed when the app starts.",
    unit: "days",
  },
  "ui.dashboard_activity_count": {
    label: "Entries on the Dashboard",
    help: "How many recent activity entries the Dashboard lists.",
    unit: "entries",
  },
};

function SettingRow({ entry, onChanged }: { entry: SettingEntry; onChanged: () => void }) {
  const id = useId();
  const [draft, setDraft] = useState(entry.value);
  // Follow the saved value when it changes (after a save, a reset, or a restore).
  useEffect(() => setDraft(entry.value), [entry.value]);
  const save = useAction(api.setSetting);
  const reset = useAction(api.deleteSetting);
  const text = TEXT[entry.key] ?? { label: entry.key, help: "", unit: "" };

  const n = Number(draft.trim());
  const valid = draft.trim() !== "" && Number.isInteger(n) && n >= entry.min && n <= entry.max;
  const dirty = draft.trim() !== entry.value;
  const busy = save.pending || reset.pending;

  const onSave = async () => {
    if (await save.run(entry.key, String(n))) onChanged();
  };
  const onReset = async () => {
    if (await reset.run(entry.key)) onChanged();
  };

  return (
    <div className="border-b border-line/60 py-3 first:pt-0 last:border-0 last:pb-0">
      <label htmlFor={id} className="text-sm font-medium">{text.label}</label>
      <div className="mt-1 flex flex-wrap items-center gap-2">
        <input
          id={id}
          type="number"
          inputMode="numeric"
          min={entry.min}
          max={entry.max}
          value={draft}
          onChange={(e) => {
            setDraft(e.target.value);
            save.reset();
          }}
          aria-describedby={`${id}-help`}
          aria-invalid={!valid}
          className="w-24 rounded-md border border-line bg-canvas px-2 py-1.5 text-sm tabular-nums"
        />
        {text.unit && <span className="text-sm text-muted">{text.unit}</span>}
        <Button variant="primary" onClick={onSave} disabled={!valid || !dirty || busy} aria-label={`Save ${text.label}`}>
          Save
        </Button>
        {!entry.isDefault && (
          <Button onClick={onReset} disabled={busy} aria-label={`Reset ${text.label} to ${entry.defaultValue}`}>
            Reset to {entry.defaultValue}
          </Button>
        )}
      </div>
      <p id={`${id}-help`} className="mt-1 text-xs text-muted">
        {text.help} Allowed: {entry.min} to {entry.max}. Default: {entry.defaultValue}.
      </p>
      {!valid && dirty && <p className="mt-1 text-xs text-warn">Enter a whole number from {entry.min} to {entry.max}.</p>}
      {save.state.status === "done" && <p role="status" className="mt-1 text-xs text-ok">Saved.</p>}
      {(save.state.status === "error" || reset.state.status === "error") && (
        <p role="alert" className="mt-1 text-xs text-danger">
          {save.state.status === "error" ? save.state.message : reset.state.status === "error" ? reset.state.message : ""}
        </p>
      )}
    </div>
  );
}

/** Preferences that are really stored in the database and survive a restart. */
export function PreferencesCard() {
  const { state, reload } = useBackend(api.listSettings);

  return (
    <Card title="Preferences">
      {state.status === "loading" && <p className="text-sm text-muted">Reading saved preferences…</p>}
      {state.status === "offline" && (
        <p className="text-sm text-muted">Preferences are only available inside the Nexora window.</p>
      )}
      {state.status === "error" && (
        <div role="alert">
          <p className="text-sm text-danger">{state.message}</p>
          <div className="mt-2">
            <Button onClick={reload}>Try again</Button>
          </div>
        </div>
      )}
      {state.status === "ready" &&
        state.data.map((entry) => (
          <SettingRow key={entry.key} entry={entry} onChanged={reload} />
        ))}
    </Card>
  );
}
