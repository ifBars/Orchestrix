import { Component, type ErrorInfo, type ReactNode } from "react";
import { AlertCircle, RefreshCw, Copy, Check, ChevronDown, ChevronUp } from "lucide-react";
import { Button } from "@/components/ui/button";

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
  errorInfo: ErrorInfo | null;
  copied: boolean;
  showDetails: boolean;
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = {
    hasError: false,
    error: null,
    errorInfo: null,
    copied: false,
    showDetails: false,
  };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error, errorInfo: null, copied: false, showDetails: false };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error("Unhandled React error:", error, errorInfo);
    this.setState({ errorInfo });
  }

  handleReload = () => {
    window.location.reload();
  };

  handleReset = () => {
    this.setState({
      hasError: false,
      error: null,
      errorInfo: null,
      copied: false,
      showDetails: false,
    });
  };

  handleCopyError = async () => {
    const { error, errorInfo } = this.state;
    if (!error) return;

    const errorText = [
      `Error: ${error.name}: ${error.message}`,
      "",
      "Stack Trace:",
      error.stack || "No stack trace available",
      "",
      "Component Stack:",
      errorInfo?.componentStack || "No component stack available",
    ].join("\n");

    try {
      await navigator.clipboard.writeText(errorText);
      this.setState({ copied: true });
      setTimeout(() => this.setState({ copied: false }), 2000);
    } catch (err) {
      console.error("Failed to copy error:", err);
    }
  };

  toggleDetails = () => {
    this.setState(prev => ({ showDetails: !prev.showDetails }));
  };

  render() {
    if (this.state.hasError) {
      const { error, errorInfo, copied, showDetails } = this.state;

      return (
        <div className="flex min-h-screen items-center justify-center bg-background p-6 text-foreground">
          <div className="w-full max-w-2xl rounded-xl border border-border bg-card p-8 shadow-lg">
            <div className="flex items-start gap-4">
              <div className="flex-shrink-0 rounded-full bg-destructive/10 p-3">
                <AlertCircle className="h-6 w-6 text-destructive" />
              </div>
              <div className="flex-1">
                <h1 className="text-xl font-semibold tracking-tight">
                  Something went wrong
                </h1>
                <p className="mt-2 text-sm text-muted-foreground">
                  The app encountered an unexpected error. You can try to recover, or copy the
                  error details below for debugging.
                </p>
              </div>
            </div>

            {error && (
              <div className="mt-6 space-y-4">
                <div className="rounded-lg border border-destructive/20 bg-destructive/5 p-4">
                  <p className="font-mono text-sm font-medium text-destructive">
                    {error.name}: {error.message}
                  </p>
                </div>

                <Button
                  variant="outline"
                  size="sm"
                  onClick={this.toggleDetails}
                  className="w-full justify-between"
                >
                  <span>View technical details</span>
                  {showDetails ? (
                    <ChevronUp className="h-4 w-4 text-muted-foreground" />
                  ) : (
                    <ChevronDown className="h-4 w-4 text-muted-foreground" />
                  )}
                </Button>

                {showDetails && (
                  <div className="mt-3 space-y-3">
                    {error.stack && (
                      <div>
                        <h3 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                          Stack Trace
                        </h3>
                        <pre className="max-h-48 overflow-auto rounded-lg bg-muted p-3 text-xs">
                          <code>{error.stack}</code>
                        </pre>
                      </div>
                    )}
                    {errorInfo?.componentStack && (
                      <div>
                        <h3 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                          Component Stack
                        </h3>
                        <pre className="max-h-48 overflow-auto rounded-lg bg-muted p-3 text-xs">
                          <code>{errorInfo.componentStack}</code>
                        </pre>
                      </div>
                    )}
                  </div>
                )}
              </div>
            )}

            <div className="mt-6 flex flex-wrap gap-3">
              <Button onClick={this.handleReload} className="gap-2">
                <RefreshCw className="h-4 w-4" />
                Reload App
              </Button>
              <Button variant="outline" onClick={this.handleCopyError} className="gap-2">
                {copied ? (
                  <>
                    <Check className="h-4 w-4" />
                    Copied!
                  </>
                ) : (
                  <>
                    <Copy className="h-4 w-4" />
                    Copy Error Details
                  </>
                )}
              </Button>
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
