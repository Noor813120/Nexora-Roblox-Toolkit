import { TriangleAlert } from "lucide-react";
import { useState } from "react";
import { api } from "../../api";
import { useBackend } from "../../hooks/useBackend";

/**
 * Shown above every page when the database needs the user's attention: it could not be opened, or
 * a damaged one was replaced at startup. Renders nothing while loading, outside the app window, or
 * when all is well.
 */
export function DatabaseBanner() {
  const { state } = useBackend(api.databaseStatus);
  const [dismissed, setDismissed] = useState(false);

  if (state.status !== "ready") return null;
  const s = state.data;

  if (!s.available) {
    return (
      <div role="alert" className="mb-4 flex items-start gap-3 rounded-card border border-danger/40 bg-surface p-3">
        <TriangleAlert aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0 text-danger" />
        <div className="text-sm">
          <p className="font-medium">Saved data is unavailable</p>
          <p className="text-muted">
            {s.error ?? "The database could not be opened."} Settings, profiles and activity cannot be saved or
            shown until this is fixed.
          </p>
        </div>
      </div>
    );
  }

  if (s.recoveredFile && !dismissed) {
    return (
      <div role="status" className="mb-4 flex items-start gap-3 rounded-card border border-warn/40 bg-surface p-3">
        <TriangleAlert aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0 text-warn" />
        <div className="flex-1 text-sm">
          <p className="font-medium">The saved data was damaged and has been replaced</p>
          <p className="text-muted">
            The damaged file was kept as <span className="font-mono text-xs">{s.recoveredFile}</span> in the
            “quarantine” folder next to the database. You can restore a backup in Settings.
          </p>
        </div>
        <button type="button" onClick={() => setDismissed(true)} className="text-sm text-accent underline">
          Dismiss
        </button>
      </div>
    );
  }
  return null;
}
