import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router";

import { i18nReady } from "./i18n";
import "./styles.css";
import { App } from "./App";
import { AuthProvider } from "./auth";
import { ConciergeProvider } from "./concierge/ConciergeProvider";
import { ConversationsProvider } from "./conversations";

// Render once the UI copy for the chosen language has arrived, so no screen flashes bare keys.
void i18nReady.finally(() =>
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <BrowserRouter>
        <AuthProvider>
          <ConversationsProvider>
            <ConciergeProvider>
              <App />
            </ConciergeProvider>
          </ConversationsProvider>
        </AuthProvider>
      </BrowserRouter>
    </StrictMode>,
  ),
);
