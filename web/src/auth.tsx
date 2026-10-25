import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";

import { api, ApiError, type User } from "./api";

interface AuthState {
  user: User | null;
  loading: boolean;
  login: (username: string, password: string) => Promise<void>;
  logout: () => Promise<void>;
  /** Updates the signed-in user after a profile change (the picture). */
  patch: (change: Partial<User>) => void;
}

const AuthContext = createContext<AuthState | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    api
      .get<User>("/auth/me")
      .then(setUser)
      .catch((e) => {
        if (!(e instanceof ApiError && e.status === 401)) console.error(e);
      })
      .finally(() => setLoading(false));
  }, []);

  const login = useCallback(async (username: string, password: string) => {
    setUser(await api.post<User>("/auth/login", { username, password }));
  }, []);

  const logout = useCallback(async () => {
    await api.post("/auth/logout");
    setUser(null);
  }, []);

  const patch = useCallback((change: Partial<User>) => setUser((u) => (u ? { ...u, ...change } : u)), []);

  return <AuthContext.Provider value={{ user, loading, login, logout, patch }}>{children}</AuthContext.Provider>;
}

export function useAuth(): AuthState {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error("useAuth outside AuthProvider");
  return ctx;
}
