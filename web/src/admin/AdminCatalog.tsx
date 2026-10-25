import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api";
import { errorMessage } from "../i18n/errors";
import { SourceBadge } from "../components/Layout";

interface ServiceAdmin {
  id: string;
  name: string;
  category: string;
  capacity: number;
  fee_jpy: number;
  available: boolean;
  notes: string;
  source_type: string;
}

interface InfoAdmin {
  topic: string;
  title: string;
  content: string;
  content_ja: string;
  content_en: string;
  source_type: string;
  updated_on: string;
}

function InfoEditor({ info, onSaved }: { info: InfoAdmin; onSaved: () => void }) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState(info);
  return (
    <details className="card">
      <summary style={{ cursor: "pointer" }}>
        <strong>{info.title}</strong> <code>{info.topic}</code> <SourceBadge type={info.source_type} /> <span className="muted small">{info.updated_on}</span>
      </summary>
      <div className="stack" style={{ marginTop: 10 }}>
        {(["content", "content_ja", "content_en"] as const).map((k) => (
          <label key={k}>
            {t(`admin.field.${k}`)}
            <textarea rows={3} value={draft[k]} onChange={(e) => setDraft({ ...draft, [k]: e.target.value })} />
          </label>
        ))}
        <div>
          <button className="btn small" onClick={() => void api.put(`/admin/catalog/info/${info.topic}`, draft).then(onSaved)}>
            {t("admin.save")}
          </button>
        </div>
      </div>
    </details>
  );
}

export function AdminCatalog() {
  const { t } = useTranslation();
  const [services, setServices] = useState<ServiceAdmin[]>([]);
  const [info, setInfo] = useState<InfoAdmin[]>([]);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    api.get<ServiceAdmin[]>("/admin/catalog/services").then(setServices).catch((e) => setError(errorMessage(e)));
    api.get<InfoAdmin[]>("/admin/catalog/info").then(setInfo).catch((e) => setError(errorMessage(e)));
  }, []);
  useEffect(load, [load]);

  const patch = (id: string, body: Partial<ServiceAdmin>) =>
    api
      .patch(`/admin/catalog/services/${id}`, body)
      .then(load)
      .catch((e) => setError(errorMessage(e)));

  return (
    <div className="stack">
      <h1>{t("admin.catalogTitle")}</h1>
      <p className="muted">{t("admin.seedNote")}</p>
      {error && <p className="error">{t("common.error", { message: error })}</p>}
      <div className="card table-wrap">
        <table>
          <thead>
            <tr>
              <th>{t("admin.field.id")}</th>
              <th>{t("planner.service")}</th>
              <th>{t("services.category")}</th>
              <th>{t("admin.field.capacity")}</th>
              <th>{t("admin.field.fee_jpy")}</th>
              <th>{t("services.available")}</th>
            </tr>
          </thead>
          <tbody>
            {services.map((s) => (
              <tr key={s.id}>
                <td>
                  <code>{s.id}</code>
                </td>
                <td>{s.name}</td>
                <td>{t(`category.${s.category}`)}</td>
                <td>
                  <input type="number" min={1} max={500} defaultValue={s.capacity} style={{ width: 90 }} onBlur={(e) => Number(e.target.value) !== s.capacity && void patch(s.id, { capacity: Number(e.target.value) })} />
                </td>
                <td>
                  <input type="number" min={0} defaultValue={s.fee_jpy} style={{ width: 100 }} onBlur={(e) => Number(e.target.value) !== s.fee_jpy && void patch(s.id, { fee_jpy: Number(e.target.value) })} />
                </td>
                <td>
                  <input type="checkbox" style={{ width: "auto" }} checked={s.available} onChange={(e) => void patch(s.id, { available: e.target.checked })} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <h2>{t("info.title")}</h2>
      {info.map((i) => (
        <InfoEditor key={i.topic + i.updated_on} info={i} onSaved={load} />
      ))}
    </div>
  );
}
