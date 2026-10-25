import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api";
import { errorMessage } from "../i18n/errors";
import { useAuth } from "../auth";
import { Icon } from "../components/Icon";
import { formatNumber } from "../format";

interface Price {
  input_usd_per_mtok: number;
  output_usd_per_mtok: number;
}

type ReplyLength = "brief" | "natural" | "detailed";
type SmallTalk = "off" | "balanced" | "relaxed";
type Reasoning = "auto" | "low" | "medium" | "high";

interface ConversationSettings {
  reply_length: ReplyLength;
  small_talk: SmallTalk;
  reasoning: Reasoning;
  temperature: number | null;
  max_output_tokens: number;
  max_steps: number;
}

interface AiSettings {
  chat_model: string;
  memory_model: string;
  embed_model: string;
  transcribe_model: string;
  memory_enabled: boolean;
  monthly_budget_usd: number | null;
  on_budget_exceeded: "block" | "warn";
  prices: Record<string, Price>;
  conversation: ConversationSettings;
}

interface Usage {
  month_start: string;
  spent_usd: number;
  monthly_budget_usd: number | null;
  blocked: boolean;
  rows: { model: string; purpose: string; calls: number; prompt_tokens: number; completion_tokens: number; cost_usd: number; unpriced_calls: number }[];
}

const MODEL_FIELDS = ["chat_model", "memory_model", "embed_model", "transcribe_model"] as const;
const FIELD_LABEL: Record<(typeof MODEL_FIELDS)[number], string> = {
  chat_model: "settingsPage.chatModel",
  memory_model: "settingsPage.memoryModel",
  embed_model: "settingsPage.embedModel",
  transcribe_model: "settingsPage.transcribeModel",
};

const REPLY_LENGTHS: Record<ReplyLength, string> = {
  brief: "settingsPage.replyLengthBrief",
  natural: "settingsPage.replyLengthNatural",
  detailed: "settingsPage.replyLengthDetailed",
};
const SMALL_TALK: Record<SmallTalk, string> = {
  off: "settingsPage.smallTalkOff",
  balanced: "settingsPage.smallTalkBalanced",
  relaxed: "settingsPage.smallTalkRelaxed",
};
const REASONING: Record<Reasoning, string> = {
  auto: "settingsPage.reasoningAuto",
  low: "settingsPage.reasoningLow",
  medium: "settingsPage.reasoningMedium",
  high: "settingsPage.reasoningHigh",
};

export function AdminSettings() {
  const { t, i18n } = useTranslation();
  const { user } = useAuth();
  const isAdmin = user?.role === "admin";
  const [settings, setSettings] = useState<AiSettings | null>(null);
  const [usage, setUsage] = useState<Usage | null>(null);
  const [newModel, setNewModel] = useState("");
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    api.get<Usage>("/admin/usage").then(setUsage).catch((e) => setError(errorMessage(e)));
    if (isAdmin) api.get<AiSettings>("/admin/settings").then(setSettings).catch((e) => setError(errorMessage(e)));
  }, [isAdmin]);
  useEffect(load, [load]);

  const save = async () => {
    if (!settings) return;
    setSaved(false);
    setError(null);
    try {
      setSettings(await api.put<AiSettings>("/admin/settings", settings));
      setSaved(true);
      load();
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  const setConversation = (patch: Partial<ConversationSettings>) => settings && setSettings({ ...settings, conversation: { ...settings.conversation, ...patch } });

  const setPrice = (model: string, key: keyof Price, value: number) =>
    settings && setSettings({ ...settings, prices: { ...settings.prices, [model]: { ...settings.prices[model], [key]: value } } });

  return (
    <div className="stack">
      <h1>{t("settingsPage.title")}</h1>
      <p className="muted">{t("settingsPage.intro")}</p>
      {error && <p className="error">{t("common.error", { message: error })}</p>}

      {usage && (
        <div className="card stack">
          <h2>{t("settingsPage.usage")}</h2>
          <p>
            {t("settingsPage.since", { date: usage.month_start })} · <strong>{t("settingsPage.spent", { spent: usage.spent_usd.toFixed(4) })}</strong> ·{" "}
            {usage.monthly_budget_usd === null ? t("settingsPage.noLimit") : t("settingsPage.limit", { limit: usage.monthly_budget_usd })}
          </p>
          {usage.blocked && <div className="banner waiting">{t("settingsPage.blocked")}</div>}
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>{t("settingsPage.model")}</th>
                  <th>{t("settingsPage.purpose")}</th>
                  <th>{t("settingsPage.calls")}</th>
                  <th>{t("settingsPage.tokens")}</th>
                  <th>{t("settingsPage.cost")}</th>
                </tr>
              </thead>
              <tbody>
                {usage.rows.map((r) => (
                  <tr key={r.model + r.purpose}>
                    <td>
                      <code>{r.model}</code>
                    </td>
                    <td>{t(`purpose.${r.purpose}`, { defaultValue: r.purpose })}</td>
                    <td>{r.calls}</td>
                    <td>
                      {formatNumber(r.prompt_tokens, i18n.language)} / {formatNumber(r.completion_tokens, i18n.language)}
                    </td>
                    <td>
                      {r.cost_usd.toFixed(4)}
                      {r.unpriced_calls > 0 && <div className="muted small">{t("settingsPage.unpriced", { n: r.unpriced_calls })}</div>}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {!isAdmin && <p className="muted">{t("settingsPage.adminOnly")}</p>}

      {settings && (
        <>
          <div className="card stack">
            <h2>{t("settingsPage.models")}</h2>
            {MODEL_FIELDS.map((f) => (
              <label key={f}>
                {t(FIELD_LABEL[f])}
                <input value={settings[f]} onChange={(e) => setSettings({ ...settings, [f]: e.target.value.trim() })} spellCheck={false} />
              </label>
            ))}
            <label style={{ display: "flex", gap: 8, alignItems: "center" }}>
              <input type="checkbox" style={{ width: "auto" }} checked={settings.memory_enabled} onChange={(e) => setSettings({ ...settings, memory_enabled: e.target.checked })} />
              {t("settingsPage.memoryEnabled")}
            </label>
          </div>

          <div className="card stack">
            <h2>{t("settingsPage.conversation")}</h2>
            <p className="muted">{t("settingsPage.conversationIntro")}</p>
            <div className="row">
              <label>
                {t("settingsPage.replyLength")}
                <select value={settings.conversation.reply_length} onChange={(e) => setConversation({ reply_length: e.target.value as ReplyLength })}>
                  {Object.entries(REPLY_LENGTHS).map(([value, key]) => (
                    <option key={value} value={value}>
                      {t(key)}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                {t("settingsPage.smallTalk")}
                <select value={settings.conversation.small_talk} onChange={(e) => setConversation({ small_talk: e.target.value as SmallTalk })}>
                  {Object.entries(SMALL_TALK).map(([value, key]) => (
                    <option key={value} value={value}>
                      {t(key)}
                    </option>
                  ))}
                </select>
              </label>
            </div>
            <p className="muted small">{t("settingsPage.smallTalkHint")}</p>
            <div className="row">
              <label>
                {t("settingsPage.reasoning")}
                <select value={settings.conversation.reasoning} onChange={(e) => setConversation({ reasoning: e.target.value as Reasoning })}>
                  {Object.entries(REASONING).map(([value, key]) => (
                    <option key={value} value={value}>
                      {t(key)}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                {t("settingsPage.temperature")}
                <input
                  type="number"
                  min={0}
                  max={2}
                  step="0.1"
                  value={settings.conversation.temperature ?? ""}
                  onChange={(e) => setConversation({ temperature: e.target.value === "" ? null : Number(e.target.value) })}
                />
              </label>
            </div>
            <p className="muted small">
              {t("settingsPage.reasoningHint")} {t("settingsPage.temperatureHint")}
            </p>
            <div className="row">
              <label>
                {t("settingsPage.maxOutputTokens")}
                <input
                  type="number"
                  min={256}
                  max={65536}
                  step={256}
                  value={settings.conversation.max_output_tokens}
                  onChange={(e) => setConversation({ max_output_tokens: Number(e.target.value) })}
                />
              </label>
              <label>
                {t("settingsPage.maxSteps")}
                <input type="number" min={1} max={12} step={1} value={settings.conversation.max_steps} onChange={(e) => setConversation({ max_steps: Number(e.target.value) })} />
              </label>
            </div>
            <p className="muted small">
              {t("settingsPage.maxOutputTokensHint")} {t("settingsPage.maxStepsHint")}
            </p>
          </div>

          <div className="card stack">
            <h2>{t("settingsPage.budget")}</h2>
            <div className="row">
              <label>
                {t("settingsPage.monthlyBudget")}
                <input
                  type="number"
                  min={0}
                  step="0.01"
                  value={settings.monthly_budget_usd ?? ""}
                  onChange={(e) => setSettings({ ...settings, monthly_budget_usd: e.target.value === "" ? null : Number(e.target.value) })}
                />
              </label>
              <label>
                {t("settingsPage.policy")}
                <select value={settings.on_budget_exceeded} onChange={(e) => setSettings({ ...settings, on_budget_exceeded: e.target.value as "block" | "warn" })}>
                  <option value="block">{t("settingsPage.policyBlock")}</option>
                  <option value="warn">{t("settingsPage.policyWarn")}</option>
                </select>
              </label>
            </div>
            <h3>{t("settingsPage.prices")}</h3>
            <div className="table-wrap">
              <table>
                <thead>
                  <tr>
                    <th>{t("settingsPage.model")}</th>
                    <th>{t("settingsPage.input")}</th>
                    <th>{t("settingsPage.output")}</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {Object.entries(settings.prices).map(([model, p]) => (
                    <tr key={model}>
                      <td>
                        <code>{model}</code>
                      </td>
                      <td>
                        <input type="number" min={0} step="0.001" value={p.input_usd_per_mtok} onChange={(e) => setPrice(model, "input_usd_per_mtok", Number(e.target.value))} />
                      </td>
                      <td>
                        <input type="number" min={0} step="0.001" value={p.output_usd_per_mtok} onChange={(e) => setPrice(model, "output_usd_per_mtok", Number(e.target.value))} />
                      </td>
                      <td>
                        <button
                          className="btn ghost small"
                          onClick={() => {
                            const prices = { ...settings.prices };
                            delete prices[model];
                            setSettings({ ...settings, prices });
                          }}
                        >
                          {t("planner.remove")}
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <div className="row">
              <input placeholder={t("settingsPage.model")} value={newModel} onChange={(e) => setNewModel(e.target.value.trim())} spellCheck={false} />
              <button
                className="btn secondary small"
                disabled={!newModel || newModel in settings.prices}
                onClick={() => {
                  setSettings({ ...settings, prices: { ...settings.prices, [newModel]: { input_usd_per_mtok: 0, output_usd_per_mtok: 0 } } });
                  setNewModel("");
                }}
              >
                <Icon name="plus" />
                {t("settingsPage.addPrice")}
              </button>
            </div>
          </div>

          <div className="row">
            <button className="btn" onClick={() => void save()}>
              {t("settingsPage.save")}
            </button>
            {saved && <span className="muted">{t("settingsPage.saved")}</span>}
          </div>
        </>
      )}
    </div>
  );
}
