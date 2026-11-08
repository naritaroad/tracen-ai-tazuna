import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api";
import { useAuth } from "../auth";
import { errorMessage } from "../i18n/errors";
import { formatDateTime } from "../format";

type McpSettings = {
  enabled: boolean;
  monthly_budget_usd: number | null;
  read_per_minute: number;
  key_read_per_minute: number;
  feedback_per_ip_day: number;
  key_feedback_per_day: number;
  feedback_global_day: number;
};
type SettingsView = { settings: McpSettings; readonly_role_configured: boolean; tools: string[] };
type Client = { id: string; name: string; scopes: string[]; created_at: string; disabled_at: string | null; last_used_at: string | null };
type Feedback = {
  id: string;
  feedback_no: string;
  client_name: string | null;
  category: string;
  text: string;
  language: string;
  related_ref: string | null;
  status: string;
  flags: string[];
  created_at: string;
  reviewer: string | null;
  review_note: string | null;
  purged: boolean;
};
type Skill = {
  name: string;
  description: string;
  tools: string[];
  mcp_tools: string[];
  surfaces: string[];
  visibility: string;
  in_registry: boolean;
  enabled: boolean;
  evals: { id: string; passed: boolean | null }[];
};
type Activity = { calls: { tool: string; status: string; calls: number }[]; feedback: { status: string; count: number }[]; mcp_embed_usd_this_month: number };

const LIMITS: (keyof McpSettings)[] = ["read_per_minute", "key_read_per_minute", "feedback_per_ip_day", "key_feedback_per_day", "feedback_global_day"];

/** Management site: remote MCP access, client keys, the feedback review queue and the built-in skills. No AI here. */
export function AdminMcp() {
  const { t, i18n } = useTranslation();
  const { user } = useAuth();
  const isAdmin = user?.role === "admin";
  const [view, setView] = useState<SettingsView | null>(null);
  const [clients, setClients] = useState<Client[]>([]);
  const [feedback, setFeedback] = useState<Feedback[]>([]);
  const [skills, setSkills] = useState<Skill[]>([]);
  const [activity, setActivity] = useState<Activity | null>(null);
  const [newKey, setNewKey] = useState<string | null>(null);
  const [clientName, setClientName] = useState("");
  const [notes, setNotes] = useState<Record<string, string>>({});
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fail = (e: unknown) => setError(errorMessage(e));
  const load = useCallback(() => {
    api.get<Feedback[]>("/admin/feedback").then(setFeedback).catch(fail);
    api.get<Skill[]>("/admin/skills").then(setSkills).catch(fail);
    api.get<Activity>("/admin/mcp/activity").then(setActivity).catch(fail);
    if (isAdmin) {
      api.get<SettingsView>("/admin/mcp/settings").then(setView).catch(fail);
      api.get<Client[]>("/admin/mcp/clients").then(setClients).catch(fail);
    }
  }, [isAdmin]);
  useEffect(load, [load]);

  const setField = (patch: Partial<McpSettings>) => view && setView({ ...view, settings: { ...view.settings, ...patch } });

  async function save() {
    if (!view) return;
    setError(null);
    try {
      await api.put("/admin/mcp/settings", view.settings);
      setSaved(true);
      window.setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      fail(e);
    }
  }

  async function createClient() {
    setError(null);
    try {
      const r = await api.post<{ key: string }>("/admin/mcp/clients", { name: clientName });
      setNewKey(r.key);
      setClientName("");
      load();
    } catch (e) {
      fail(e);
    }
  }

  async function act(path: string, body: unknown = {}) {
    setError(null);
    try {
      await api.post(path, body);
      load();
    } catch (e) {
      fail(e);
    }
  }

  async function toggleSkill(s: Skill) {
    setError(null);
    try {
      await api.put(`/admin/skills/${s.name}`, { enabled: !s.enabled });
      load();
    } catch (e) {
      fail(e);
    }
  }

  return (
    <div className="stack">
      <h1>{t("mcpPage.title")}</h1>
      <p className="muted">{t("mcpPage.intro")}</p>
      {error && <p className="error">{t("common.error", { message: error })}</p>}

      {!isAdmin && <p className="muted">{t("settingsPage.adminOnly")}</p>}
      {view && (
        <div className="card stack">
          <h2>{t("mcpPage.remote")}</h2>
          {!view.readonly_role_configured && <p className="notice error">{t("mcpPage.noRole")}</p>}
          <label style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <input type="checkbox" style={{ width: "auto" }} checked={view.settings.enabled} onChange={(e) => setField({ enabled: e.target.checked })} />
            {t("mcpPage.enabled")}
          </label>
          <p className="muted small">{t("mcpPage.tools", { list: view.tools.join(", ") })}</p>
          <div className="row">
            <label>
              {t("mcpPage.budget")}
              <input
                type="number"
                min={0}
                step={0.1}
                value={view.settings.monthly_budget_usd ?? ""}
                onChange={(e) => setField({ monthly_budget_usd: e.target.value === "" ? null : Number(e.target.value) })}
              />
            </label>
            {LIMITS.map((k) => (
              <label key={k}>
                {t(`mcpPage.limit.${k}`)}
                <input type="number" min={0} value={view.settings[k] as number} onChange={(e) => setField({ [k]: Number(e.target.value) } as Partial<McpSettings>)} />
              </label>
            ))}
          </div>
          <div className="row">
            <button className="btn primary" onClick={() => void save()}>
              {t("mcpPage.save")}
            </button>
            {saved && <span className="muted">{t("mcpPage.saved")}</span>}
          </div>
        </div>
      )}

      {activity && (
        <div className="card stack">
          <h2>{t("mcpPage.activity")}</h2>
          <p className="muted small">{t("mcpPage.spent", { usd: activity.mcp_embed_usd_this_month.toFixed(4) })}</p>
          {activity.calls.length === 0 ? (
            <p className="muted">{t("mcpPage.noCalls")}</p>
          ) : (
            <div className="table-wrap">
              <table>
                <thead>
                  <tr>
                    <th>{t("mcpPage.tool")}</th>
                    <th>{t("mcpPage.status")}</th>
                    <th>{t("mcpPage.calls")}</th>
                  </tr>
                </thead>
                <tbody>
                  {activity.calls.map((c) => (
                    <tr key={`${c.tool}-${c.status}`}>
                      <td>
                        <code>{c.tool}</code>
                      </td>
                      <td>{t(`mcpPage.callStatus.${c.status}`, { defaultValue: c.status })}</td>
                      <td>{c.calls}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      )}

      {isAdmin && (
        <div className="card stack">
          <h2>{t("mcpPage.clients")}</h2>
          <p className="muted small">{t("mcpPage.clientsIntro")}</p>
          {newKey && (
            <p className="notice">
              {t("mcpPage.newKey")} <code>{newKey}</code>
            </p>
          )}
          <div className="row">
            <input value={clientName} onChange={(e) => setClientName(e.target.value)} placeholder={t("mcpPage.clientName")} aria-label={t("mcpPage.clientName")} maxLength={80} />
            <button className="btn" onClick={() => void createClient()} disabled={!clientName.trim()}>
              {t("mcpPage.createKey")}
            </button>
          </div>
          {clients.map((c) => (
            <div key={c.id} className="row" style={{ alignItems: "center" }}>
              <strong>{c.name}</strong>
              <span className="muted small">
                {c.last_used_at ? t("mcpPage.lastUsed", { when: formatDateTime(c.last_used_at, i18n.language) }) : t("mcpPage.neverUsed")}
              </span>
              {c.disabled_at ? (
                <span className="badge">{t("mcpPage.disabled")}</span>
              ) : (
                <button className="btn" onClick={() => void act(`/admin/mcp/clients/${c.id}/disable`)}>
                  {t("mcpPage.disable")}
                </button>
              )}
            </div>
          ))}
        </div>
      )}

      <div className="card stack">
        <h2>{t("mcpPage.feedback")}</h2>
        <p className="muted small">{t("mcpPage.feedbackIntro")}</p>
        {feedback.length === 0 && <p className="muted">{t("mcpPage.noFeedback")}</p>}
        {feedback.map((f) => (
          <article key={f.id} className="stack feedback-item" style={{ gap: 6 }}>
            <div className="row" style={{ alignItems: "center" }}>
              <strong>{f.feedback_no}</strong>
              <span className={`badge ${f.status}`}>{t(`mcpPage.fbStatus.${f.status}`)}</span>
              <span className="muted small">
                {t(`mcpPage.category.${f.category}`)} · {f.language} · {formatDateTime(f.created_at, i18n.language)}
                {f.client_name ? ` · ${f.client_name}` : ""}
              </span>
              {f.flags.includes("possible_injection") && <span className="badge bad">{t("mcpPage.flagInjection")}</span>}
            </div>
            {/* Plain text only: feedback is shown to people, never rendered as markup. */}
            <p style={{ margin: 0, whiteSpace: "pre-wrap" }}>{f.purged ? t("mcpPage.purged") : f.text}</p>
            {f.related_ref && <span className="muted small">{t("mcpPage.related", { ref: f.related_ref })}</span>}
            {f.status === "pending_review" ? (
              <div className="row">
                <input
                  value={notes[f.id] ?? ""}
                  onChange={(e) => setNotes({ ...notes, [f.id]: e.target.value })}
                  placeholder={t("mcpPage.note")}
                  aria-label={t("mcpPage.note")}
                  maxLength={500}
                />
                {(["accepted", "merged", "rejected"] as const).map((s) => (
                  <button key={s} className="btn" onClick={() => void act(`/admin/feedback/${f.id}/review`, { status: s, note: notes[f.id] })}>
                    {t(`mcpPage.review.${s}`)}
                  </button>
                ))}
              </div>
            ) : (
              <span className="muted small">
                {f.reviewer ?? ""}
                {f.review_note ? ` · ${f.review_note}` : ""}
              </span>
            )}
          </article>
        ))}
      </div>

      <div className="card stack">
        <h2>{t("mcpPage.skills")}</h2>
        <p className="muted small">{t("mcpPage.skillsIntro")}</p>
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>{t("mcpPage.skill")}</th>
                <th>{t("mcpPage.where")}</th>
                <th>{t("mcpPage.tests")}</th>
                <th>{t("mcpPage.state")}</th>
              </tr>
            </thead>
            <tbody>
              {skills.map((s) => {
                const ran = s.evals.filter((e) => e.passed !== null);
                const ok = ran.filter((e) => e.passed).length;
                return (
                  <tr key={s.name}>
                    <td>
                      <code>{s.name}</code>
                      <div className="muted small">{s.description}</div>
                    </td>
                    <td className="small">{s.surfaces.map((x) => t(`mcpPage.surface.${x}`)).join(" · ")}</td>
                    <td className="small">{ran.length ? t("mcpPage.passed", { ok, total: ran.length }) : t("mcpPage.notRun")}</td>
                    <td>
                      <div className="row" style={{ alignItems: "center", flexWrap: "nowrap" }}>
                        <span className={`badge ${s.enabled ? "ok" : ""}`}>{s.enabled ? t("mcpPage.on") : t("mcpPage.off")}</span>
                        {isAdmin && s.in_registry && (
                          <button className="btn" onClick={() => void toggleSkill(s)}>
                            {s.enabled ? t("mcpPage.switchOff") : t("mcpPage.switchOn")}
                          </button>
                        )}
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
