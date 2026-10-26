import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api, type PlanResult, type ServiceView } from "../api";
import { errorMessage } from "../i18n/errors";
import { Icon } from "../components/Icon";
import { formatList, formatNumber, formatYen, hoursMinutes } from "../format";

interface Row {
  service: string;
  sessions: number;
  minutes: string;
}

export function PlannerPage() {
  const { t, i18n } = useTranslation();
  const [services, setServices] = useState<ServiceView[]>([]);
  const [rows, setRows] = useState<Row[]>([{ service: "stretch", sessions: 1, minutes: "" }, { service: "turf_basic", sessions: 2, minutes: "" }]);
  const [date, setDate] = useState("");
  const [start, setStart] = useState("");
  const [result, setResult] = useState<PlanResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.get<ServiceView[]>(`/catalog/services?locale=${i18n.language}`).then(setServices).catch((e) => setError(errorMessage(e)));
  }, [i18n.language]);

  const update = (i: number, patch: Partial<Row>) => setRows((rs) => rs.map((r, j) => (j === i ? { ...r, ...patch } : r)));

  const calculate = async () => {
    setError(null);
    try {
      const r = await api.post<PlanResult>("/catalog/plan", {
        items: rows.map((r) => ({ service: r.service, sessions: r.sessions, ...(r.minutes ? { minutes_per_session: Number(r.minutes) } : {}) })),
        date: date || undefined,
        start_time: date && start ? start : undefined,
        locale: i18n.language,
      });
      setResult(r);
    } catch (e) {
      setResult(null);
      setError(errorMessage(e));
    }
  };

  return (
    <div className="stack">
      <h1>{t("planner.title")}</h1>
      <p className="muted">{t("planner.intro")}</p>
      <div className="card stack">
        {rows.map((r, i) => (
          <div className="row" key={i}>
            <label style={{ flex: "2 1 220px" }}>
              {t("planner.service")}
              <select value={r.service} onChange={(e) => update(i, { service: e.target.value })}>
                {services.map((s) => (
                  <option key={s.id} value={s.id}>
                    {s.name}
                    {s.available ? "" : ` (${t("services.unavailable")})`}
                  </option>
                ))}
              </select>
            </label>
            <label>
              {t("planner.sessions")}
              <input type="number" min={1} max={50} value={r.sessions} onChange={(e) => update(i, { sessions: Math.max(1, Number(e.target.value) || 1) })} />
            </label>
            <label>
              {t("planner.minutes")}
              <input type="number" min={5} max={480} value={r.minutes} onChange={(e) => update(i, { minutes: e.target.value })} />
            </label>
            <button className="btn ghost small" disabled={rows.length === 1} onClick={() => setRows((rs) => rs.filter((_, j) => j !== i))}>
              {t("planner.remove")}
            </button>
          </div>
        ))}
        <div>
          <button className="btn secondary small" disabled={rows.length >= 20} onClick={() => setRows((rs) => [...rs, { service: services[0]?.id ?? "", sessions: 1, minutes: "" }])}>
            <Icon name="plus" />
            {t("planner.add")}
          </button>
        </div>
        <div className="row">
          <label>
            {t("planner.date")}
            <input type="date" value={date} onChange={(e) => setDate(e.target.value)} />
          </label>
          <label>
            {t("planner.start")}
            <input type="time" value={start} disabled={!date} onChange={(e) => setStart(e.target.value)} />
          </label>
          <button className="btn" onClick={() => void calculate()}>
            {t("planner.calculate")}
          </button>
        </div>
      </div>

      {error && <p className="error">{t("common.error", { message: error })}</p>}

      {result && (
        <div className="card stack">
          {!result.complete && (
            <div>
              <h2>{t("planner.problems")}</h2>
              <ul>
                {result.problems.map((p) => (
                  <li key={p.item}>{t(`planProblem.${p.kind}`, { item: p.item, candidates: formatList(p.candidates, i18n.language, "disjunction"), defaultValue: p.kind })}</li>
                ))}
              </ul>
            </div>
          )}
          {result.complete && result.total_minutes !== null && (
            <div>
              <h2>{t("planner.total", { min: result.total_minutes, h: hoursMinutes(result.total_minutes) })}</h2>
              <p>{t("planner.fee", { n: formatNumber(result.total_fee_jpy ?? 0, i18n.language) })}</p>
              <div className="table-wrap">
                <table>
                  <tbody>
                    {result.lines.map((l) => (
                      <tr key={l.service_id}>
                        <td>{l.name}</td>
                        <td>
                          {l.minutes_per_session} × {l.sessions} = {t("common.minutes", { n: l.minutes })}
                        </td>
                        <td>{formatYen(l.fee_jpy, i18n.language)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
              <p className="muted small">
                {t("planner.formula")}{t("common.colon")}<code>{result.formula}</code>
              </p>
            </div>
          )}
          {result.schedule && (
            <div>
              <h3>{t("planner.schedule")}</h3>
              <ul>
                {result.schedule.blocks.map((b) => (
                  <li key={b.start}>
                    {b.start} – {b.end.slice(11)} {b.name}
                  </li>
                ))}
              </ul>
            </div>
          )}
          {result.complete && (
            <div>
              <h3>{t("planner.conflicts")}</h3>
              {result.conflicts.length === 0 ? (
                <p className="muted">{t("planner.noConflicts")}</p>
              ) : (
                <ul>
                  {result.conflicts.map((c, i) => (
                    <li key={i}>
                      <span className="badge bad">{t(`conflict.${c.kind}`)}</span> {c.event_title ?? ""} <span className="muted small">{c.detail}</span>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
