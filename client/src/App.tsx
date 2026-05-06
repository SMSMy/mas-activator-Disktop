import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import NotFound from "@/pages/NotFound";
import { Route, Switch } from "wouter";
import ErrorBoundary from "./components/ErrorBoundary";
import { UpdateDialog } from "./components/UpdateDialog";
import { ThemeProvider } from "./contexts/ThemeContext";
import { useUpdateCheck } from "./hooks/useUpdateCheck";
import Home from "./pages/Home";


function Router() {
  return (
    <Switch>
      <Route path={"/"} component={Home} />
      <Route path={"/404"} component={NotFound} />
      <Route component={NotFound} />
    </Switch>
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
          <UpdateDialog
            info={updateInfo}
            open={showDialog}
            onOpenChange={setShowDialog}
          />
        </TooltipProvider>
      </ThemeProvider>
    </ErrorBoundary>
  );
}

export default App;
