import { useId, type ReactNode } from "react";

interface CardProps {
  /** Rendered as an `h2` and used as the section's accessible name. */
  title?: string;
  /** Right-aligned controls in the header, e.g. a refresh button. */
  actions?: ReactNode;
  className?: string;
  children: ReactNode;
}

/** The one surface every panel uses: flat, 1px border, no shadow. */
export function Card({ title, actions, className = "", children }: CardProps) {
  const headingId = useId();
  return (
    <section
      aria-labelledby={title ? headingId : undefined}
      className={`rounded-card border border-line bg-surface p-4 ${className}`}
    >
      {(title || actions) && (
        <header className="mb-3 flex items-center justify-between gap-3">
          {title && (
            <h2 id={headingId} className="text-sm font-semibold text-ink">
              {title}
            </h2>
          )}
          {actions}
        </header>
      )}
      {children}
    </section>
  );
}
