import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api, type Ticket, type TicketEvent } from "../api";
import { errorMessage } from "../i18n/errors";
import { formatDateTime, newId } from "../format";

export const REASONS = ["not_in_data", "exception_request", "complaint", "asked_for_director", "other"] as const;

function TicketCard({ ticket }: { ticket: Ticket }) {
  const { t, i18n } = useTranslation();
  const [events, setEvents] = useState<TicketEvent[] | null>(null);
  return (
    <details
      className="card"
      onToggle={(e) => {
        if ((e.target as HTMLDetailsElement).open && !events)
          api.get<{ events: TicketEvent[] }>(`/director/tickets/${ticket.id}`).then((d) => setEvents(d.events)).catch(() => setEvents([]));
      }}
    >
      <summary style={{ cursor: "pointer" }}>
        <strong>{ticket.ticket_no}</strong> <span className={`badge ${ticket.status}`}>{t(`ticketStatus.${ticket.status}`)}</span>{" "}
        <span className="muted small">
          {t(`reason.${ticket.reason}`)} · {formatDateTime(ticket.created_at, i18n.language)}
        </span>
        <div className="small">{ticket.summary}</div>
      </summary>
      <div className="timeline" style={{ marginTop: 12 }}>
        <div className="muted small">{t("director.events")}</div>
        {events?.map((ev) => (
          <div key={ev.id} className={`ev ${ev.kind}`}>
            <div className="small muted">
              {t(`ticketEvent.${ev.kind}`)}
              {ev.kind === "reply" || ev.kind === "accepted" ? ` · ${t("chat.director")}` : ""} · {formatDateTime(ev.created_at, i18n.language)}
            </div>
            {ev.body && <div style={{ whiteSpace: "pre-wrap" }}>{ev.body}</div>}
          </div>
        ))}
      </div>
    </details>
  );
}

export function DirectorPage() {
  const { t } = useTranslation();
  const [tickets, setTickets] = useState<Ticket[] | null>(null);
  const [reason, setReason] = useState<string>("other");
  const [summary, setSummary] = useState("");
  const [requestId, setRequestId] = useState(newId);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    api.get<Ticket[]>("/director/tickets").then(setTickets).catch((e) => setError(errorMessage(e)));
  }, []);
  useEffect(load, [load]);

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      // The same request id on a retry returns the same case instead of a second one.
      const ticket = await api.post<Ticket>("/director/tickets", { reason, summary, request_id: requestId });
      setMessage(t("director.sent", { no: ticket.ticket_no }));
      setSummary("");
      setRequestId(newId());
      load();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="stack">
      <h1>{t("director.title")}</h1>
      <p className="muted">{t("director.intro")}</p>
      <div className="card stack">
        <label>
          {t("director.reason")}
          <select value={reason} onChange={(e) => setReason(e.target.value)}>
            {REASONS.map((r) => (
              <option key={r} value={r}>
                {t(`reason.${r}`)}
              </option>
            ))}
          </select>
        </label>
        <label>
          {t("director.summary")}
          <textarea rows={4} maxLength={2000} value={summary} onChange={(e) => setSummary(e.target.value)} />
        </label>
        <div>
          <button className="btn" disabled={busy || !summary.trim()} onClick={() => void submit()}>
            {t("director.submit")}
          </button>
        </div>
        {message && <p>{message}</p>}
        {error && <p className="error">{t("common.error", { message: error })}</p>}
      </div>
      <h2>{t("director.mine")}</h2>
      {tickets && tickets.length === 0 && <div className="card empty">{t("director.empty")}</div>}
      {tickets?.map((tk) => (
        <TicketCard key={tk.id} ticket={tk} />
      ))}
    </div>
  );
}
