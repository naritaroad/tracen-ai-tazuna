import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api, type Ticket, type TicketEvent } from "../api";
import { errorMessage } from "../i18n/errors";
import { formatDateTime } from "../format";

interface Detail {
  ticket: Ticket;
  events: TicketEvent[];
}

export function AdminInbox() {
  const { t, i18n } = useTranslation();
  const [tickets, setTickets] = useState<Ticket[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [detail, setDetail] = useState<Detail | null>(null);
  const [reply, setReply] = useState("");
  const [error, setError] = useState<string | null>(null);

  const loadList = useCallback(() => {
    api.get<Ticket[]>("/admin/director/tickets").then(setTickets).catch((e) => setError(errorMessage(e)));
  }, []);
  const loadDetail = useCallback((id: string) => {
    api.get<Detail>(`/admin/director/tickets/${id}`).then(setDetail).catch((e) => setError(errorMessage(e)));
  }, []);

  useEffect(() => {
    loadList();
    const id = window.setInterval(loadList, 10000);
    return () => window.clearInterval(id);
  }, [loadList]);
  useEffect(() => {
    if (selected) loadDetail(selected);
  }, [selected, loadDetail]);

  const act = async (action: "accept" | "resolve" | "return" | "reply") => {
    if (!selected) return;
    setError(null);
    try {
      await api.post(`/admin/director/tickets/${selected}/${action}`, action === "reply" ? { body: reply } : {});
      if (action === "reply") setReply("");
      loadDetail(selected);
      loadList();
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  const tk = detail?.ticket;
  return (
    <div className="stack">
      <h1>{t("admin.inboxTitle")}</h1>
      <p className="muted">{t("admin.persona")}</p>
      {error && <p className="error">{t("common.error", { message: error })}</p>}
      <div className="inbox">
        <div className="stack" style={{ gap: 8 }}>
          {tickets.map((x) => (
            <button key={x.id} className={`ticket-item ${x.id === selected ? "active" : ""}`} onClick={() => setSelected(x.id)}>
              <span>
                <strong>{x.ticket_no}</strong> <span className={`badge ${x.status}`}>{t(`ticketStatus.${x.status}`)}</span>
              </span>
              <span className="small">{x.summary.slice(0, 80)}</span>
              <span className="muted small">
                {x.requester_name} · {formatDateTime(x.created_at, i18n.language)}
              </span>
            </button>
          ))}
          {tickets.length === 0 && <div className="card empty">{t("director.empty")}</div>}
        </div>
        <div className="card stack">
          {!tk && <p className="muted">{t("admin.select")}</p>}
          {tk && (
            <>
              <div className="row" style={{ alignItems: "center" }}>
                <h2 style={{ margin: 0 }}>{tk.ticket_no}</h2>
                <span className={`badge ${tk.status}`} style={{ flex: "0 0 auto" }}>
                  {t(`ticketStatus.${tk.status}`)}
                </span>
              </div>
              <div className="small muted">
                {t("admin.requester")}{t("common.colon")}{tk.requester_name} · {t("director.reason")}{t("common.colon")}{t(`reason.${tk.reason}`)} · {t("admin.origin")}{t("common.colon")}
                {tk.origin === "agent" ? t("admin.originAgent") : t("admin.originForm")}
                {tk.assignee_name && ` · ${t("admin.assignee")}${t("common.colon")}${tk.assignee_name}`}
              </div>
              <p style={{ whiteSpace: "pre-wrap" }}>{tk.summary}</p>
              <div className="timeline">
                {detail.events.map((ev) => (
                  <div key={ev.id} className={`ev ${ev.kind}`}>
                    <div className="small muted">
                      {t(`ticketEvent.${ev.kind}`)} · {ev.actor_name ?? t("common.none")} · {formatDateTime(ev.created_at, i18n.language)}
                    </div>
                    {ev.body && <div style={{ whiteSpace: "pre-wrap" }}>{ev.body}</div>}
                  </div>
                ))}
              </div>
              {tk.status === "waiting" && (
                <div className="row">
                  <button className="btn" onClick={() => void act("accept")}>
                    {t("admin.accept")}
                  </button>
                  <button className="btn ghost" onClick={() => void act("return")}>
                    {t("admin.hand_back")}
                  </button>
                </div>
              )}
              {tk.status === "in_progress" && (
                <div className="stack">
                  <textarea rows={3} maxLength={4000} placeholder={t("admin.replyPlaceholder")} value={reply} onChange={(e) => setReply(e.target.value)} />
                  <div className="row">
                    <button className="btn" disabled={!reply.trim()} onClick={() => void act("reply")}>
                      {t("admin.reply")}
                    </button>
                    <button className="btn secondary" onClick={() => void act("resolve")}>
                      {t("admin.resolve")}
                    </button>
                    <button className="btn ghost" onClick={() => void act("return")}>
                      {t("admin.hand_back")}
                    </button>
                  </div>
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
