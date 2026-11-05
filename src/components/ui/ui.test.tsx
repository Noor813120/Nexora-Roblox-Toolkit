import { fireEvent, render, screen } from "@testing-library/react";
import { Inbox } from "lucide-react";
import { describe, expect, it, vi } from "vitest";
import { Button } from "./Button";
import { Card } from "./Card";
import { EmptyState } from "./EmptyState";
import { ErrorBoundary } from "./ErrorBoundary";
import { StatCard } from "./StatCard";

describe("EmptyState", () => {
  it("shows the title, description and phase label, and no button unless given one", () => {
    render(<EmptyState icon={Inbox} title="Nothing here" description="Coming later." phase="Planned for Phase 9" />);
    expect(screen.getByRole("heading", { name: "Nothing here" })).toBeInTheDocument();
    expect(screen.getByText("Coming later.")).toBeInTheDocument();
    expect(screen.getByText("Planned for Phase 9")).toBeInTheDocument();
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("renders a real action when one is supplied", () => {
    render(<EmptyState icon={Inbox} title="T" description="D" action={<Button>Do it</Button>} />);
    expect(screen.getByRole("button", { name: "Do it" })).toBeInTheDocument();
  });
});

describe("Card", () => {
  it("names its section by its heading", () => {
    render(<Card title="Automation">body</Card>);
    expect(screen.getByRole("region", { name: "Automation" })).toBeInTheDocument();
  });
});

describe("StatCard", () => {
  it("shows label, value, badge and detail", () => {
    render(<StatCard label="CPU" value="8 cores" badge={{ text: "Partial" }} detail="Core count only." />);
    expect(screen.getByRole("heading", { name: "CPU" })).toBeInTheDocument();
    expect(screen.getByText("8 cores")).toBeInTheDocument();
    expect(screen.getByText("Partial")).toBeInTheDocument();
    expect(screen.getByText("Core count only.")).toBeInTheDocument();
  });
});

describe("Button", () => {
  it("defaults to type=button so it never submits a form", () => {
    render(<Button>Go</Button>);
    expect(screen.getByRole("button", { name: "Go" })).toHaveAttribute("type", "button");
  });

  it("calls onClick and respects disabled", () => {
    const onClick = vi.fn();
    const { rerender } = render(<Button onClick={onClick}>Go</Button>);
    fireEvent.click(screen.getByRole("button"));
    expect(onClick).toHaveBeenCalledTimes(1);
    rerender(
      <Button onClick={onClick} disabled>
        Go
      </Button>,
    );
    fireEvent.click(screen.getByRole("button"));
    expect(onClick).toHaveBeenCalledTimes(1);
  });
});

describe("ErrorBoundary", () => {
  function Bomb({ explode }: { explode: boolean }) {
    if (explode) throw new Error("kaboom");
    return <p>all good</p>;
  }

  it("shows a recoverable fallback instead of crashing, and logs the error", () => {
    const log = vi.spyOn(console, "error").mockImplementation(() => {});
    render(
      <ErrorBoundary>
        <Bomb explode />
      </ErrorBoundary>,
    );
    expect(screen.getByRole("alert")).toHaveTextContent("This page ran into a problem");
    expect(screen.getByRole("button", { name: "Try again" })).toBeInTheDocument();
    expect(log).toHaveBeenCalled();
    log.mockRestore();
  });

  it("renders its children when nothing fails", () => {
    render(
      <ErrorBoundary>
        <Bomb explode={false} />
      </ErrorBoundary>,
    );
    expect(screen.getByText("all good")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).toBeNull();
  });
});
