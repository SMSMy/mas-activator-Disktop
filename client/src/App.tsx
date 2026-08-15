import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import NotFound from "@/pages/NotFound";
import { Route, Switch } from "wouter";
import ErrorBoundary from "./components/ErrorBoundary";
import { UpdateDialog } from "./components/UpdateDialog";
import { ThemeProvider, useTheme } from "./contexts/ThemeContext";
import { useUpdateCheck } from "./hooks/useUpdateCheck";
import Home from "./pages/Home";
import type { UpdateInfo } from "./hooks/useUpdateCheck";

function Router() {
  return (
    <Switch>
      <Route path={"/"} component={Home} />
      <Route path={"/404"} component={NotFound} />
      <Route component={NotFound} />
    </Switch>
  );
}

function ThemedUpdateDialog({
  updateInfo,
  showDialog,
  setShowDialog,
}: {
  updateInfo: UpdateInfo | null;
  showDialog: boolean;
  setShowDialog: (open: boolean) => void;
}) {
  const { theme } = useTheme();
  return (
    <UpdateDialog
      info={updateInfo}
      open={showDialog}
      onOpenChange={setShowDialog}
      isDark={theme === "dark"}
    />
  );
}

function App() {
  const { updateInfo, showDialog, setShowDialog } = useUpdateCheck();

  return (
    <ErrorBoundary>
      <ThemeProvider
        defaultTheme="light"
        switchable
      >
        <TooltipProvider>
          <Toaster />
          <Router />
          <ThemedUpdateDialog
            updateInfo={updateInfo}
            showDialog={showDialog}
            setShowDialog={setShowDialog}
          />
        </TooltipProvider>
      </ThemeProvider>
    </ErrorBoundary>
  );
}

export default App;
