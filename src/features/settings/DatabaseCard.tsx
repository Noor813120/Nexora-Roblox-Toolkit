import { DatabaseBackup, DatabaseZap } from "lucide-react";
import { useState } from "react";
import { api, type BackupOutcome, type RestoreOutcome } from "../../api";
import { Button } from "../../components/ui/Button";
import { Card } from "../../components/ui/Card";
import { StatusPill } from "../../components/ui/StatusPill";
import { useAction } from "../../hooks/useAction";
import { useBackend } from "../../hooks/useBackend";
import { formatBytes } from "../../lib/format";

/** Where the data lives, how healthy it is, and the backup/restore controls. */
export function DatabaseCard({ onRestored }: { onRestored: () => void }) {
  const status = useBackend(api.databaseStatus);
  const backup = useAction(api.backupDatabase);
  const restore = useAction(api.restoreDatabase);
  const [backupResult, setBackupResult] = useState<BackupOutcome | null>(null);
  const [restoreResult, setRestoreResult] = useState<RestoreOutcome | null>(null);
  const [confirming, setConfirming] = useState(false);

  const available = status.state.status === "ready" && status.state.data.available;

  const onBackup = async () => {
    setRestoreResult(null);
    const result = await backup.run();
    if (result) {
      setBackupResult(result);
      status.reload();
    }
  };

  const onRestore = async () => {
    setBackupResult(null);
    const result = await restore.run();
    setConfirming(false);
    if (result) {
      setRestoreResult(result);
      if (result.restored) {
        status.reload();
        onRestored();
      }
    }
  };

  return (
    <Card title="Database">
      {status.state.status === "loading" && <p className="text-sm text-muted">Reading database status…</p>}
      {status.state.status === "offline" && (
        <p className="text-sm text-muted">Database information is only available inside the RoALT Advanced window.</p>
      )}
      {status.state.status === "error" && <p role="alert" className="text-sm text-danger">{status.state.message}</p>}
      {status.state.status === "ready" && (
        <>
          <div className="mb-3">
            {status.state.data.available ? <StatusPill tone="ok">Working</StatusPill> : <StatusPill tone="danger">Unavailable</StatusPill>}
          </div>
          {status.state.data.error && <p role="alert" className="mb-3 text-sm text-danger">{status.state.data.error}</p>}
          <dl className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1.5 text-sm">
            <dt className="text-muted">Location</dt>
            <dd className="break-all">{status.state.data.location ?? "—"}</dd>
            {status.state.data.available && (
              <>
                <dt className="text-muted">Schema version</dt>
                <dd>{status.state.data.schemaVersion ?? "—"}</dd>
                <dt className="text-muted">Journal mode</dt>
                <dd>{status.state.data.journalMode ?? "—"}</dd>
                <dt className="text-muted">Main file size</dt>
                <dd>{status.state.data.sizeBytes === null ? "—" : formatBytes(status.state.data.sizeBytes)}</dd>
              </>
            )}
          </dl>
        </>
      )}

      <div className="mt-4 border-t border-line pt-4">
        <p className="mb-3 text-sm text-muted">
          A backup is one file you choose the location of. Nothing is uploaded anywhere. Restoring replaces all
          saved data with the backup; the current data is kept as a safety copy first.
        </p>
        <div className="flex flex-wrap gap-2">
          <Button onClick={onBackup} disabled={!available || backup.pending || restore.pending}>
            <DatabaseBackup aria-hidden="true" className="h-4 w-4" />
            {backup.pending ? "Waiting for you to choose a file…" : "Create backup…"}
          </Button>
          <Button onClick={() => setConfirming(true)} disabled={!available || confirming || backup.pending || restore.pending}>
            <DatabaseZap aria-hidden="true" className="h-4 w-4" />
            Restore from backup…
          </Button>
        </div>

        {confirming && (
          <div role="alertdialog" aria-labelledby="restore-title" aria-describedby="restore-desc" className="mt-3 rounded-md border border-warn/40 p-3">
            <h3 id="restore-title" className="text-sm font-semibold">Restore from a backup?</h3>
            <p id="restore-desc" className="mt-1 text-sm text-muted">
              Everything currently saved (preferences, profiles, automation settings and activity) will be replaced
              by the contents of the backup file. A copy of what is there now is kept in the app's data folder.
            </p>
            <div className="mt-3 flex gap-2">
              <Button variant="primary" onClick={onRestore} disabled={restore.pending}>
                {restore.pending ? "Restoring…" : "Choose a backup file…"}
              </Button>
              <Button onClick={() => setConfirming(false)} disabled={restore.pending}>Cancel</Button>
            </div>
          </div>
        )}

        {backupResult && (
          <p role="status" className="mt-3 text-sm text-ok">
            {backupResult.saved
              ? `Backup saved as ${backupResult.fileName ?? "the chosen file"} (${formatBytes(backupResult.sizeBytes ?? 0)}).`
              : "No file was chosen. Nothing was saved."}
          </p>
        )}
        {restoreResult && (
          <p role="status" className={`mt-3 text-sm ${restoreResult.restored ? "text-ok" : "text-muted"}`}>
            {restoreResult.restored
              ? `Restored. Your previous data was kept as ${restoreResult.safetyCopy ?? "a safety copy"} in the app's data folder.${
                  restoreResult.migratedVersions.length > 0 ? " The backup was upgraded to the current format." : ""
                }`
              : "No file was chosen. Nothing was changed."}
          </p>
        )}
        {backup.state.status === "error" && <p role="alert" className="mt-3 text-sm text-danger">{backup.state.message}</p>}
        {restore.state.status === "error" && <p role="alert" className="mt-3 text-sm text-danger">{restore.state.message}</p>}
      </div>
    </Card>
  );
}
