import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api, type InfoView } from "../api";
import { errorMessage } from "../i18n/errors";
import { SourceBadge } from "../components/Layout";

export function InfoPage() {
  const { t, i18n } = useTranslation();
  const [items, setItems] = useState<InfoView[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.get<InfoView[]>(`/catalog/info?locale=${i18n.language}`).then(setItems).catch((e) => setError(errorMessage(e)));
  }, [i18n.language]);

  return (
    <div className="stack">
      <h1>{t("info.title")}</h1>
      {error && <p className="error">{t("common.error", { message: error })}</p>}
      {!items && !error && <p className="muted">{t("common.loading")}</p>}
      <div className="grid-cards">
        {items?.map((i) => (
          <article key={i.topic} className="card stack" style={{ gap: 8 }}>
            <h2 style={{ margin: 0 }}>{i.title}</h2>
            <p style={{ margin: 0 }}>{i.content}</p>
            <div className="row muted small" style={{ alignItems: "center" }}>
              <SourceBadge type={i.source_type} />
              <span>{t("info.updated", { date: i.updated_on })}</span>
            </div>
            <div className="muted small">{i.source_ref}</div>
          </article>
        ))}
      </div>
    </div>
  );
}
