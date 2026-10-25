import { useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import { Navigate, useLocation, useNavigate } from "react-router";

import { useAuth } from "../auth";
import { Controls, Mark } from "../components/Layout";

export function LoginPage() {
  const { t } = useTranslation();
  const { user, login } = useAuth();
  const navigate = useNavigate();
  const location = useLocation();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [failed, setFailed] = useState(false);
  const [busy, setBusy] = useState(false);
  const from = (location.state as { from?: string } | null)?.from ?? "/";

  if (user) return <Navigate to={from} replace />;

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setFailed(false);
    try {
      await login(username, password);
      navigate(from, { replace: true });
    } catch {
      setFailed(true);
    } finally {
      setBusy(false);
    }
  };

  return (
    <main className="login-wrap">
      <form className="card login-card stack" onSubmit={submit}>
        <div className="row" style={{ alignItems: "center" }}>
          <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <Mark />
            <div>
              <h1 style={{ margin: 0 }}>{t("login.title")}</h1>
              <div className="muted small">{t("app.name")}</div>
            </div>
          </div>
          <Controls />
        </div>
        <label>
          {t("login.username")}
          <input
            autoComplete="username"
            autoCapitalize="none"
            autoCorrect="off"
            spellCheck={false}
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            required
          />
        </label>
        <label>
          {t("login.password")}
          <input type="password" autoComplete="current-password" value={password} onChange={(e) => setPassword(e.target.value)} required />
        </label>
        {failed && <p className="error">{t("login.failed")}</p>}
        <button className="btn" disabled={busy}>
          {t("login.submit")}
        </button>
        <p className="muted small">{t("login.hint")}</p>
      </form>
    </main>
  );
}
