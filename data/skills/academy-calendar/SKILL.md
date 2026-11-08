---
name: academy-calendar
description: "List Tracen Academy calendar entries (classes, exams, events, maintenance, workshops) and the G1/G2 race schedule between two dates."
---

# Academy calendar

## Routing

Dates, exams, events, maintenance, G1/G2 race schedule, "what is on" -> list_academy_events. Resolve words like 'next Thursday' to a date
using TODAY below before calling.

## Limits

- At most 62 days per call.
- Races take a half month and never close a facility.

## External use

Call `list_public_events` with `date_from` and `date_to` (YYYY-MM-DD, academy time, at most 62 days). Races are game data (`source_type` community, see `source_ref`).
