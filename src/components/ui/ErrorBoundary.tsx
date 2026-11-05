import { Component, type ErrorInfo, type ReactNode } from "react";
import { TriangleAlert } from "lucide-react";
import { Button } from "./Button";
import { EmptyState } from "./EmptyState";

interface Props {
  children: ReactNode;
}

interface State {
  failed: boolean;
}

/**
 * Catches a render error in one page so the sidebar keeps working. The shell remounts it with a
 * new `key` on every navigation, so moving to another page always clears the error.
 */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { failed: false };

  static getDerivedStateFromError(): State {
    return { failed: true };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    // Logged to the webview console only. No data leaves the app.
    console.error("Page crashed:", error, info.componentStack);
  }

  render(): ReactNode {
    if (!this.state.failed) return this.props.children;
    return (
      <div role="alert">
        <EmptyState
          icon={TriangleAlert}
          title="This page ran into a problem"
          description="The rest of the app is still working. You can try the page again or open another one from the sidebar."
          action={<Button onClick={() => this.setState({ failed: false })}>Try again</Button>}
        />
      </div>
    );
  }
}
