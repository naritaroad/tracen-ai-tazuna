---
name: public-feedback
description: "Send non-personal feedback about the public Tracen Academy data (an error, missing information, a suggestion) to a human review queue."
---

# Public feedback

## Limits

- No contact details, names, IDs or anything private: the server refuses feedback that looks like personal data.
- Feedback waits for a person; it never changes data, prompts or memory by itself.

## External use

Only when the user wants to report an error, missing information or a suggestion about the public data, call `submit_public_feedback` once with a category, 10–1000 characters of text, the language and a new `idempotency_key`. Never include names, contact details, student numbers or account information. The reply is a receipt number; do not promise that the feedback will be adopted.
