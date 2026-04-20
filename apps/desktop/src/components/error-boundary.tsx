/**
 * Minimal React error boundary.
 *
 * Wrap major UI surfaces (project workspace, detached windows) so a
 * single render-time crash doesn't take the whole app down — the user
 * should always be able to navigate away, open a different tab, or
 * reload the config.
 *
 * Deliberately simple: no telemetry, no Sentry, no fancy recovery.
 * A better story (structured reporting, "report issue" link) lands in
 * Phase 4. The important thing today is to stop a busted CodeMirror /
 * mongo parser / stale binding from turning into a blank screen.
 */
import { Component, type ErrorInfo, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { AlertTriangleIcon } from "lucide-react";

interface ErrorBoundaryProps {
  /** Shown in the fallback header. */
  label?: string;
  /** Override the whole fallback UI. */
  fallback?: (err: Error, reset: () => void) => ReactNode;
  children: ReactNode;
}

interface ErrorBoundaryState {
  error: Error | null;
}

export class ErrorBoundary extends Component<
  ErrorBoundaryProps,
  ErrorBoundaryState
> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    // One place for render-error logging; keep it noisy during dev so
    // we notice regressions instead of silently showing the fallback.
    console.error("[ErrorBoundary]", this.props.label ?? "unknown", error, info);
  }

  reset = () => this.setState({ error: null });

  render() {
    const { error } = this.state;
    if (!error) return this.props.children;

    if (this.props.fallback) {
      return this.props.fallback(error, this.reset);
    }

    return (
      <div className="flex flex-col items-center justify-center gap-3 p-6 text-center h-full min-h-[200px]">
        <AlertTriangleIcon className="size-8 text-destructive" />
        <div className="space-y-1">
          <h2 className="text-sm font-semibold">
            {this.props.label ?? "Something went wrong"}
          </h2>
          <p className="text-xs text-muted-foreground max-w-sm">
            {error.message || "An unexpected error occurred."}
          </p>
        </div>
        <Button size="sm" variant="outline" onClick={this.reset}>
          Try again
        </Button>
      </div>
    );
  }
}
