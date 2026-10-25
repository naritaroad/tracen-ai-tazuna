import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api";
import { errorMessage } from "../i18n/errors";
import { formatDateTime } from "../format";

interface RunAudit {
  id: string;
  trainer: string;
  model: string;
  locale: string;
  status: string;
  steps: number;
  error_code: string | null;
  tools: string[];
  started_at: string;
}

export function AdminRuns() {
  const { t, i18n } = useTranslation();
  const [runs, setRuns] = useState<RunAudit[]>([]);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    api.get<RunAudit[]>("/admin/runs").then(setRuns).catch((e) => setError(errorMessage(e)));
  }, []);
  return (
    <div className="stack">
      <h1>{t("admin.runsTitle")}</h1>
      {error && <p className="error">{t("common.error", { message: error })}</p>}
      <div className="card table-wrap">
        <table>
          <thead>
            <tr>
              <th />
              <th>{t("admin.trainer")}</th>
              <th>{t("chat.model")}</th>
              <th>{t("admin.steps")}</th>
              <th>{t("admin.tools")}</th>
            </tr>
          </thead>
          <tbody>
            {runs.map((r) => (
              <tr key={r.id}>
                <td className="small">
                  {formatDateTime(r.started_at, i18n.language)}{" "}
                  <span className={`badge ${r.status === "completed" ? "ok" : r.status === "running" ? "" : "bad"}`}>
                    {r.error_code ? t(`runError.${r.error_code}`, { defaultValue: r.error_code }) : t(`runStatus.${r.status}`, { defaultValue: r.status })}
                  </span>
                </td>
                <td>{r.trainer}</td>
                <td>
                  <code>{r.model}</code> <span className="muted small">{r.locale}</span>
                </td>
                <td>{r.steps}</td>
                <td className="small">{r.tools.length ? r.tools.map((n) => t(`tools.${n}`, { defaultValue: n })).join(" → ") : t("common.none")}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
