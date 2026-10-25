import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api, type ServiceView } from "../api";
import { errorMessage } from "../i18n/errors";
import { SourceBadge } from "../components/Layout";
import { formatNumber } from "../format";

export const CATEGORIES = ["track", "equipment", "analysis", "room", "workshop", "support"] as const;

export function ServicesPage() {
  const { t, i18n } = useTranslation();
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState("");
  const [onlyAvailable, setOnlyAvailable] = useState(false);
  const [items, setItems] = useState<ServiceView[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const params = new URLSearchParams({ locale: i18n.language });
    if (query.trim()) params.set("query", query.trim());
    if (category) params.set("category", category);
    if (onlyAvailable) params.set("only_available", "true");
    const timer = window.setTimeout(() => {
      api.get<ServiceView[]>(`/catalog/services?${params}`).then(setItems).catch((e) => setError(errorMessage(e)));
    }, 200);
    return () => window.clearTimeout(timer);
  }, [query, category, onlyAvailable, i18n.language]);

  return (
    <div className="stack">
      <h1>{t("services.title")}</h1>
      <div className="card row">
        <label>
          {t("services.query")}
          <input value={query} onChange={(e) => setQuery(e.target.value)} />
        </label>
        <label>
          {t("services.category")}
          <select value={category} onChange={(e) => setCategory(e.target.value)}>
            <option value="">{t("services.anyCategory")}</option>
            {CATEGORIES.map((c) => (
              <option key={c} value={c}>
                {t(`category.${c}`)}
              </option>
            ))}
          </select>
        </label>
        <label style={{ flexDirection: "row", display: "flex", alignItems: "center", gap: 8 }}>
          <input type="checkbox" style={{ width: "auto" }} checked={onlyAvailable} onChange={(e) => setOnlyAvailable(e.target.checked)} />
          {t("services.onlyAvailable")}
        </label>
      </div>
      {error && <p className="error">{t("common.error", { message: error })}</p>}
      {items && items.length === 0 && <div className="card empty">{t("services.empty")}</div>}
      <div className="grid-cards">
        {items?.map((s) => (
          <article key={s.id} className="card stack" style={{ gap: 6 }}>
            <div className="row" style={{ alignItems: "center", justifyContent: "space-between" }}>
              <h2 style={{ margin: 0, flex: "1 1 auto" }}>{s.name}</h2>
              <span className={`badge ${s.available ? "ok" : "bad"}`} style={{ flex: "0 0 auto" }}>
                {s.available ? t("services.available") : t("services.unavailable")}
              </span>
            </div>
            <div className="muted small">
              {t(`category.${s.category}`)} · {s.location}
            </div>
            <div className="small">
              {t("services.unit", { n: s.unit_minutes })} · {t("services.capacity", { n: s.capacity })} ·{" "}
              {s.fee_jpy === 0 ? t("services.free") : t("services.fee", { n: formatNumber(s.fee_jpy, i18n.language) })}
            </div>
            {s.notes && <div className="muted small">{s.notes}</div>}
            <div>
              <SourceBadge type={s.source_type} />
            </div>
          </article>
        ))}
      </div>
    </div>
  );
}
