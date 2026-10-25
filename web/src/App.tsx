import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Navigate, Route, Routes, useLocation } from "react-router";

import { AdminCatalog } from "./admin/AdminCatalog";
import { AdminInbox } from "./admin/AdminInbox";
import { AdminRuns } from "./admin/AdminRuns";
import { AdminSettings } from "./admin/AdminSettings";
import { AdminUsers } from "./admin/AdminUsers";
import { useAuth } from "./auth";
import { Layout } from "./components/Layout";
import { CalendarPage } from "./pages/CalendarPage";
import { ChatPage } from "./pages/ChatPage";
import { DirectorPage } from "./pages/DirectorPage";
import { InfoPage } from "./pages/InfoPage";
import { LoginPage } from "./pages/LoginPage";
import { MemoryPage } from "./pages/MemoryPage";
import { PlannerPage } from "./pages/PlannerPage";
import { ServicesPage } from "./pages/ServicesPage";
import { RouteMemory } from "./routeMemory";

function RequireUser({ staff = false, children }: { staff?: boolean; children: ReactNode }) {
  const { user, loading } = useAuth();
  const { t } = useTranslation();
  const location = useLocation();
  if (loading) return <div className="empty">{t("common.loading")}</div>;
  if (!user) return <Navigate to="/login" replace state={{ from: location.pathname }} />;
  if (staff && user.role === "trainer") return <Navigate to="/" replace />;
  return <>{children}</>;
}

const trainer = (page: ReactNode) => (
  <RequireUser>
    <Layout>{page}</Layout>
  </RequireUser>
);

// The chat page fills the window and scrolls its own messages.
const chat = (page: ReactNode) => (
  <RequireUser>
    <Layout chat>{page}</Layout>
  </RequireUser>
);

const staff = (page: ReactNode) => (
  <RequireUser staff>
    <Layout admin>{page}</Layout>
  </RequireUser>
);

export function App() {
  return (
    <>
      <RouteMemory />
      <Routes>
      <Route path="/login" element={<LoginPage />} />
      <Route path="/" element={chat(<ChatPage />)} />
      <Route path="/c/:conversationId" element={chat(<ChatPage />)} />
      <Route path="/info" element={trainer(<InfoPage />)} />
      <Route path="/calendar" element={trainer(<CalendarPage />)} />
      <Route path="/services" element={trainer(<ServicesPage />)} />
      <Route path="/planner" element={trainer(<PlannerPage />)} />
      <Route path="/director" element={trainer(<DirectorPage />)} />
      <Route path="/memory" element={trainer(<MemoryPage />)} />
      <Route path="/admin" element={staff(<AdminInbox />)} />
      <Route path="/admin/catalog" element={staff(<AdminCatalog />)} />
      <Route path="/admin/runs" element={staff(<AdminRuns />)} />
      <Route path="/admin/users" element={staff(<AdminUsers />)} />
      <Route path="/admin/settings" element={staff(<AdminSettings />)} />
      <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </>
  );
}
