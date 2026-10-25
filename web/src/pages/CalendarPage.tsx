import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api, type EventView, type RaceView, type ServiceView } from "../api";
import { errorMessage } from "../i18n/errors";
import { SourceBadge } from "../components/Layout";
import { formatList, formatNumber, todayInAcademy } from "../format";

type RaceFilter = "all" | "g1" | "none";
const RACE_FILTERS: RaceFilter[] = ["all", "g1", "none"];

function showRace(race: RaceView | undefined, filter: RaceFilter): boolean {
  return !race || filter === "all" || (filter === "g1" && race.grade === "G1");
}

function RaceDetails({ race }: { race: RaceView }) {
  const { t, i18n } = useTranslation();
  return (
    <div className="small muted">
      <div>
        {t("race.detail", {
          racecourse: t(`racecourse.${race.racecourse}`),
          surface: t(`race.surface.${race.surface}`),
          distance: formatNumber(race.distance_m, i18n.language),
          direction: t(`race.direction.${race.direction}`),
        })}
      </div>
      <div>
        {t("race.classes", { classes: formatList(race.classes.map((c) => t(`race.class.${c}`)), i18n.language) })}
        {race.source_ref && (
          <>
            {" · "}
            <a href={race.source_ref} target="_blank" rel="noreferrer">
              {t("race.source")}
            </a>
          </>
        )}
      </div>
    </div>
  );
}

export function CalendarPage() {
  const { t, i18n } = useTranslation();
  const [from, setFrom] = useState(todayInAcademy());
  const [to, setTo] = useState(todayInAcademy(30));
  const [raceFilter, setRaceFilter] = useState<RaceFilter>("all");
  const [events, setEvents] = useState<EventView[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Events name the facilities they affect by id; show the facility names in the current language.
  const [serviceNames, setServiceNames] = useState<Map<string, string>>(new Map());

  useEffect(() => {
    api
      .get<ServiceView[]>(`/catalog/services?locale=${i18n.language}`)
      .then((list) => setServiceNames(new Map(list.map((s) => [s.id, s.name]))))
      .catch(() => setServiceNames(new Map()));
  }, [i18n.language]);

  const load = useCallback(() => {
    setError(null);
    api
      .get<EventView[]>(`/catalog/events?from=${from}&to=${to}&locale=${i18n.language}`)
      .then(setEvents)
      .catch((e) => setError(errorMessage(e)));
  }, [from, to, i18n.language]);

  useEffect(load, [i18n.language]); // eslint-disable-line react-hooks/exhaustive-deps

  const shown = events?.filter((e) => showRace(e.race, raceFilter)) ?? null;

  return (
    <div className="stack">
      <h1>{t("calendar.title")}</h1>
      <div className="card row">
        <label>
          {t("calendar.from")}
          <input type="date" value={from} onChange={(e) => setFrom(e.target.value)} />
        </label>
        <label>
          {t("calendar.to")}
          <input type="date" value={to} onChange={(e) => setTo(e.target.value)} />
        </label>
        <label>
          {t("calendar.races")}
          <select value={raceFilter} onChange={(e) => setRaceFilter(e.target.value as RaceFilter)}>
            {RACE_FILTERS.map((f) => (
              <option key={f} value={f}>
                {t(`calendar.raceFilter.${f}`)}
              </option>
            ))}
          </select>
        </label>
        <button className="btn" onClick={load}>
          {t("calendar.search")}
        </button>
      </div>
      {raceFilter !== "none" && <p className="small muted">{t("race.halfNote")}</p>}
      {error && <p className="error">{t("common.error", { message: error })}</p>}
      {shown && shown.length === 0 && <div className="card empty">{t("calendar.empty")}</div>}
      {shown && shown.length > 0 && (
        <div className="card table-wrap">
          <table>
            <thead>
              <tr>
                <th>{t("calendar.from")}</th>
                <th>{t("calendar.to")}</th>
                <th />
                <th>{t("calendar.affects")}</th>
              </tr>
            </thead>
            <tbody>
              {shown.map((e) => (
                <tr key={e.id}>
                  <td>{e.starts_at}</td>
                  <td>{e.ends_at}</td>
                  <td>
                    <strong>{e.title}</strong>
                    <div className="row small" style={{ gap: 6 }}>
                      {e.race && <span className={`badge ${e.race.grade.toLowerCase()}`}>{e.race.grade}</span>}
                      <span className="badge">{t(`kind.${e.kind}`)}</span>
                      <SourceBadge type={e.source_type} />
                    </div>
                    {e.race && <RaceDetails race={e.race} />}
                  </td>
                  <td className="small">
                    {e.affects_services.length === 0
                      ? t("calendar.none")
                      : e.affects_services.includes("all")
                        ? t("calendar.all")
                        : formatList(
                            e.affects_services.map((id) => serviceNames.get(id) ?? id),
                            i18n.language,
                          )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
