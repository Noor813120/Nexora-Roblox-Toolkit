/** Placeholder shown while a lazily loaded page downloads. Pulses only if motion is allowed. */
export function PageSkeleton() {
  return (
    <div role="status" aria-label="Loading page" className="space-y-4 motion-safe:animate-pulse">
      <div className="h-7 w-48 rounded bg-raised" />
      <div className="h-4 w-80 rounded bg-raised" />
      <div className="grid grid-cols-1 gap-4 pt-2 sm:grid-cols-2 xl:grid-cols-4">
        {[0, 1, 2, 3].map((i) => (
          <div key={i} className="h-28 rounded-card bg-raised" />
        ))}
      </div>
    </div>
  );
}
