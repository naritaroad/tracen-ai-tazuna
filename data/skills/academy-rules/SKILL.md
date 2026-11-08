---
name: academy-rules
description: "Look up Tracen Academy rules and procedures in the Trainer Handbook and the academy information by meaning. Use for rules, conditions, exceptions, penalties, appeals and 'can I / what happens if' questions."
---

# Academy rules

## Routing

Rules, procedures, conditions, exceptions, penalties, appeals, "can I / may I / what happens if" -> search_academy_rules.
Answer only from the sections it returns and name the handbook section you used (for example §7.3).

## Limits

- The results are the closest sections, not necessarily answers. If none of them answers the question, it is not found.
- Handbook sections never give numbers that the academy data owns (slot lengths, fees, deadlines); use the other skills for those.
- Only sections the reader may see are searched: trainers see public and trainer sections; outside agents see public ones only.

## External use

Call `search_public_rules` with the user's question in their own words. Answer only from the returned sections and cite the section number (for example §7.3). If `found` is false, say the handbook has nothing on it; do not guess.
