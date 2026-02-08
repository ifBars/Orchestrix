import { Component, type ErrorInfo, type ReactNode } from "react";

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  hasError: boolean;
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { hasError: false };

  static getDerivedStateFromError(): ErrorBoundaryState {
    return { hasError: true };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error("Unhandled React error", error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="flex min-h-screen items-center justify-center bg-background px-6 text-foreground">
          <div className="max-w-md rounded-xl border border-border bg-card p-6 text-center shadow-lg">
            <h1 className="text-xl font-semibold tracking-tight">Something went wrong</h1>
            <p className="mt-2 text-sm text-muted-foreground">
              The app hit an unexpected error. Reload the window to recover.
            </p>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
