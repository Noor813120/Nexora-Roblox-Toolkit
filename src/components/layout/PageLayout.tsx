import type { ReactNode } from "react";

interface PageLayoutProps {
  title: string;
  description?: string;
  /** Controls aligned to the right of the title, e.g. a refresh button. */
  actions?: ReactNode;
  children: ReactNode;
}

/** Shared page frame: one `h1`, an optional description and actions, then the page content. */
export function PageLayout({ title, description, actions, children }: PageLayoutProps) {
  return (
    <div className="ui-fade-in">
      <header className="mb-6 flex items-start justify-between gap-4">
        <div>
          <h1 className="text-xl font-semibold">{title}</h1>
          {description && <p className="mt-1 text-sm text-muted">{description}</p>}
        </div>
        {actions}
      </header>
      {children}
    </div>
  );
}
