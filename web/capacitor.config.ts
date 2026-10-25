import type { CapacitorConfig } from "@capacitor/cli";

// The Android app shows the same site as the browser and talks to the same Rust API (Q14 A).
// It loads the site from the API server, so login cookies and /api stay on one origin.
// USB testing: `adb reverse tcp:8080 tcp:8080` makes the phone's localhost:8080 the dev machine's API,
// which also counts as a secure context for the microphone.
// A deployed server: set CAP_SERVER_URL=https://your-host before `npx cap sync android`.
const serverUrl = process.env.CAP_SERVER_URL ?? "http://localhost:8080";

const config: CapacitorConfig = {
  appId: "dev.tracen.aas",
  appName: "Tracen AAS",
  webDir: "dist",
  server: {
    url: serverUrl,
    cleartext: serverUrl.startsWith("http://"),
  },
  android: {
    // Plain http only for the USB / LAN development address above.
    allowMixedContent: false,
  },
};

export default config;
