import React, { useEffect, useState } from "react";
import {
  BrowserRouter as Router,
  Routes,
  Route,
  Navigate,
  Link,
  useNavigate,
} from "react-router-dom";
import { AuthProvider, useAuth } from "./AuthContext";
import { Login } from "./pages/Login";
import { ProviderSetup } from "./pages/ProviderSetup";
import { ResearchDashboard } from "./pages/ResearchDashboard";
import { SwarmDashboard } from "./pages/SwarmDashboard";
import { TextAdventure } from "./pages/TextAdventure";
import { SecondBrainDashboard } from "./pages/SecondBrainDashboard";
import { ConnectorsDashboard } from "./pages/ConnectorsDashboard";
import { OpsWorkspace } from "./pages/OpsWorkspace";
import { RepoAgentDashboard } from "./pages/RepoAgentDashboard";
import { IncidentTriageDashboard } from "./pages/IncidentTriageDashboard";
import { DataExtractionDashboard } from "./pages/DataExtractionDashboard";
import { TicketTriageDashboard } from "./pages/TicketTriageDashboard";
import { ScheduledWatchDashboard } from "./pages/ScheduledWatchDashboard";
import { ContentCreatorDashboard } from "./pages/ContentCreatorDashboard";
import { HtmlExtractorDashboard } from "./pages/HtmlExtractorDashboard";
import {
  LayoutDashboard,
  Users,
  MessageSquareQuote,
  BrainCircuit,
  LogOut,
  Cable,
  ShieldCheck,
  Settings,
  GitPullRequest,
  FileWarning,
  DatabaseZap,
  Ticket,
  Clock,
  PenTool,
  Code,
} from "lucide-react";

const ProtectedRoute = ({ children }: { children: React.ReactNode }) => {
  const { token, isLoading } = useAuth();
  if (isLoading) return <div className="text-white p-8">Loading session...</div>;
  return token ? <>{children}</> : <Navigate to="/" replace />;
};

const ProviderReadyRoute = ({ children }: { children: React.ReactNode }) => {
  const { providerConfigured, providerLoading } = useAuth();
  if (providerLoading) return <div className="text-white p-8">Loading provider config...</div>;
  return providerConfigured ? <>{children}</> : <Navigate to="/setup" replace />;
};

const NavigationLayout = ({ children }: { children: React.ReactNode }) => {
  const { logout } = useAuth();
  const navigate = useNavigate();
  const [showSetupHint, setShowSetupHint] = useState(false);

  useEffect(() => {
    const key = "tandem_portal_setup_hint_dismissed";
    if (!localStorage.getItem(key)) {
      setShowSetupHint(true);
    }
  }, []);

  const dismissSetupHint = () => {
    localStorage.setItem("tandem_portal_setup_hint_dismissed", "1");
    setShowSetupHint(false);
  };

  return (
    <div className="flex h-screen bg-gray-950">
      {/* Sidebar */}
      <div className="w-64 bg-gray-900 border-r border-gray-800 flex flex-col">
        <div className="p-4 border-b border-gray-800">
          <h1 className="text-xl font-bold text-white flex items-center gap-2">
            <BrainCircuit className="text-emerald-500" />
            Tandem Portal
          </h1>
        </div>
        <nav className="flex-1 p-4 space-y-2 overflow-y-auto">
          <Link
            to="/setup"
            className="flex items-center gap-3 text-gray-300 hover:text-white hover:bg-gray-800 p-2 rounded-md"
          >
            <Settings size={20} /> Provider Setup
          </Link>
          <Link
            to="/research"
            className="flex items-center gap-3 text-gray-300 hover:text-white hover:bg-gray-800 p-2 rounded-md"
          >
            <LayoutDashboard size={20} /> Research
          </Link>
          <Link
            to="/repo"
            className="flex items-center gap-3 text-gray-300 hover:text-white hover:bg-gray-800 p-2 rounded-md"
          >
            <GitPullRequest size={20} /> Repo Agent
          </Link>
          <Link
            to="/triage"
            className="flex items-center gap-3 text-gray-300 hover:text-white hover:bg-gray-800 p-2 rounded-md"
          >
            <FileWarning size={20} /> Incident Triage
          </Link>
          <Link
            to="/data"
            className="flex items-center gap-3 text-gray-300 hover:text-white hover:bg-gray-800 p-2 rounded-md"
          >
            <DatabaseZap size={20} /> Data Extraction
          </Link>
          <Link
            to="/tickets"
            className="flex items-center gap-3 text-gray-300 hover:text-white hover:bg-gray-800 p-2 rounded-md"
          >
            <Ticket size={20} /> Ticket Triage
          </Link>
          <Link
            to="/watch"
            className="flex items-center gap-3 text-gray-300 hover:text-white hover:bg-gray-800 p-2 rounded-md"
          >
            <Clock size={20} /> Scheduled Watch
          </Link>
          <Link
            to="/content"
            className="flex items-center gap-3 text-gray-300 hover:text-white hover:bg-gray-800 p-2 rounded-md"
          >
            <PenTool size={20} /> Content Creator
          </Link>
          <Link
            to="/html"
            className="flex items-center gap-3 text-gray-300 hover:text-white hover:bg-gray-800 p-2 rounded-md"
          >
            <Code size={20} /> HTML Escape-Hatch
          </Link>
          <Link
            to="/swarm"
            className="flex items-center gap-3 text-gray-300 hover:text-white hover:bg-gray-800 p-2 rounded-md"
          >
            <Users size={20} /> Agent Swarm
          </Link>
          <Link
            to="/adventure"
            className="flex items-center gap-3 text-gray-300 hover:text-white hover:bg-gray-800 p-2 rounded-md"
          >
            <MessageSquareQuote size={20} /> Adventure
          </Link>
          <Link
            to="/second-brain"
            className="flex items-center gap-3 text-gray-300 hover:text-white hover:bg-gray-800 p-2 rounded-md"
          >
            <BrainCircuit size={20} /> Second Brain
          </Link>
          <Link
            to="/channels"
            className="flex items-center gap-3 text-gray-300 hover:text-white hover:bg-gray-800 p-2 rounded-md"
          >
            <Cable size={20} /> Connectors
          </Link>
          <Link
            to="/ops"
            className="flex items-center gap-3 text-gray-300 hover:text-white hover:bg-gray-800 p-2 rounded-md"
          >
            <ShieldCheck size={20} /> Ops
          </Link>
        </nav>
        <div className="p-4 border-t border-gray-800">
          <button
            onClick={logout}
            className="flex items-center gap-3 text-gray-400 hover:text-white w-full p-2 rounded-md"
          >
            <LogOut size={20} /> Disconnect
          </button>
        </div>
      </div>

      {/* Main Content */}
      <div className="flex-1 overflow-auto">{children}</div>

      {showSetupHint && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
          <div className="w-full max-w-md rounded-xl border border-gray-700 bg-gray-900 p-5 shadow-2xl">
            <div className="flex items-center gap-2 text-white">
              <Settings className="text-emerald-500" size={20} />
              <h3 className="text-lg font-semibold">Provider Setup</h3>
            </div>
            <p className="mt-3 text-sm text-gray-300">
              Configure your default provider and model from{" "}
              <span className="font-medium">Provider Setup</span> so example runs use the expected
              model every time.
            </p>
            <div className="mt-4 flex items-center justify-end gap-2">
              <button
                onClick={dismissSetupHint}
                className="rounded-md border border-gray-700 px-3 py-2 text-sm text-gray-300 hover:bg-gray-800"
              >
                Dismiss
              </button>
              <button
                onClick={() => {
                  dismissSetupHint();
                  navigate("/setup");
                }}
                className="rounded-md bg-emerald-600 px-3 py-2 text-sm font-medium text-white hover:bg-emerald-500"
              >
                Open Setup
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default function App() {
  return (
    <AuthProvider>
      <Router>
        <Routes>
          <Route path="/" element={<Login />} />
          <Route
            path="/setup"
            element={
              <ProtectedRoute>
                <ProviderSetup />
              </ProtectedRoute>
            }
          />

          <Route
            path="/research"
            element={
              <ProtectedRoute>
                <ProviderReadyRoute>
                  <NavigationLayout>
                    <ResearchDashboard />
                  </NavigationLayout>
                </ProviderReadyRoute>
              </ProtectedRoute>
            }
          />
          <Route
            path="/repo"
            element={
              <ProtectedRoute>
                <ProviderReadyRoute>
                  <NavigationLayout>
                    <RepoAgentDashboard />
                  </NavigationLayout>
                </ProviderReadyRoute>
              </ProtectedRoute>
            }
          />
          <Route
            path="/triage"
            element={
              <ProtectedRoute>
                <ProviderReadyRoute>
                  <NavigationLayout>
                    <IncidentTriageDashboard />
                  </NavigationLayout>
                </ProviderReadyRoute>
              </ProtectedRoute>
            }
          />
          <Route
            path="/data"
            element={
              <ProtectedRoute>
                <ProviderReadyRoute>
                  <NavigationLayout>
                    <DataExtractionDashboard />
                  </NavigationLayout>
                </ProviderReadyRoute>
              </ProtectedRoute>
            }
          />
          <Route
            path="/tickets"
            element={
              <ProtectedRoute>
                <ProviderReadyRoute>
                  <NavigationLayout>
                    <TicketTriageDashboard />
                  </NavigationLayout>
                </ProviderReadyRoute>
              </ProtectedRoute>
            }
          />
          <Route
            path="/watch"
            element={
              <ProtectedRoute>
                <ProviderReadyRoute>
                  <NavigationLayout>
                    <ScheduledWatchDashboard />
                  </NavigationLayout>
                </ProviderReadyRoute>
              </ProtectedRoute>
            }
          />
          <Route
            path="/content"
            element={
              <ProtectedRoute>
                <ProviderReadyRoute>
                  <NavigationLayout>
                    <ContentCreatorDashboard />
                  </NavigationLayout>
                </ProviderReadyRoute>
              </ProtectedRoute>
            }
          />
          <Route
            path="/html"
            element={
              <ProtectedRoute>
                <ProviderReadyRoute>
                  <NavigationLayout>
                    <HtmlExtractorDashboard />
                  </NavigationLayout>
                </ProviderReadyRoute>
              </ProtectedRoute>
            }
          />
          <Route
            path="/swarm"
            element={
              <ProtectedRoute>
                <ProviderReadyRoute>
                  <NavigationLayout>
                    <SwarmDashboard />
                  </NavigationLayout>
                </ProviderReadyRoute>
              </ProtectedRoute>
            }
          />
          <Route
            path="/adventure"
            element={
              <ProtectedRoute>
                <ProviderReadyRoute>
                  <NavigationLayout>
                    <TextAdventure />
                  </NavigationLayout>
                </ProviderReadyRoute>
              </ProtectedRoute>
            }
          />
          <Route
            path="/second-brain"
            element={
              <ProtectedRoute>
                <ProviderReadyRoute>
                  <NavigationLayout>
                    <SecondBrainDashboard />
                  </NavigationLayout>
                </ProviderReadyRoute>
              </ProtectedRoute>
            }
          />
          <Route
            path="/ops"
            element={
              <ProtectedRoute>
                <NavigationLayout>
                  <OpsWorkspace />
                </NavigationLayout>
              </ProtectedRoute>
            }
          />
          <Route
            path="/channels"
            element={
              <ProtectedRoute>
                <ProviderReadyRoute>
                  <NavigationLayout>
                    <ConnectorsDashboard />
                  </NavigationLayout>
                </ProviderReadyRoute>
              </ProtectedRoute>
            }
          />
        </Routes>
      </Router>
    </AuthProvider>
  );
}
