# 對話串流事件（SSE）

`POST /api/v1/conversations/{id}/messages` 回傳 `text/event-stream`。每個事件：

```
event: <type>
data: {"v":1,"seq":<n>,"type":"<type>","data":{...}}
```

`v` 是事件格式版本。`seq` 在同一個串流內由 1 遞增。

| type | data | 說明 |
|---|---|---|
| `message.accepted` | `{message_id, conversation_id}` | 使用者訊息已存檔 |
| `run.started` | `{run_id, model, locale}` | Agent 開始 |
| `avatar.intent` | 見 [avatar-contract.md](../character/avatar-contract.md) | 角色狀態 |
| `tool.started` | `{call_id, step, name, arguments}` | 開始執行工具 |
| `tool.completed` | `{call_id, step, name, status, result, duration_ms}` | `status`：`ok`／`rejected`／`error` |
| `message.delta` | `{text}` | 回覆文字片段（首版整段一次送出） |
| `message.completed` | `{message_id, content, run_id}` | 助理訊息已存檔 |
| `handoff.updated` | `{ticket_no, status, conversation_status}` | 理事長室案件建立或狀態改變 |
| `handoff.active` | `{ticket_no}` | 對話已由真人接手，本訊息不交給 AI |
| `run.completed` | `{run_id, steps, usage}` | 結束 |
| `run.failed` | `{run_id, code, message}` | `code`：`max_steps`／`timeout`／`model_error`／`not_configured` |

順序保證：`message.accepted` 最先；`run.completed` 或 `run.failed` 最後（真人接手時以 `handoff.active` 結束）。用戶端斷線不取消 run，結果仍會存檔，重新整理後以 `GET /api/v1/conversations/{id}` 取得。
