# Platform metadata rules

Last verified: 2026-08-28. Recheck official sources before final compliance
claim.

## Apple App Store

Official references:

- [App information](https://developer.apple.com/help/app-store-connect/reference/app-information/app-information)
- [Platform version information](https://developer.apple.com/help/app-store-connect/reference/app-information/platform-version-information)

| Field | Limit | Unit | Runtime behavior |
|---|---:|---|---|
| Name | 30 | characters | required |
| Subtitle | 30 | characters | checked when present |
| Promotional text | 170 | characters | checked when present |
| Description | 4,000 | characters | required |
| Keywords | 100 | UTF-8 bytes | required; entries under three characters fail |
| What's new | 4,000 | characters | checked when present |

Apple says app/company names should not be duplicated in keyword list and other
apps' or companies' names are not allowed. Runtime flags direct duplicates
available in input; trademark, relevance, and policy review remain human work.

## Google Play

Official reference:

- [Create and set up your app](https://support.google.com/googleplay/android-developer/answer/9859152)

| Field | Limit | Unit | Runtime behavior |
|---|---:|---|---|
| App name | 30 | characters | required |
| Short description | 80 | characters | required |
| Full description | 4,000 | characters | required |

Google warns repetitive or irrelevant keyword use can cause suspension. Runtime
checks structure and limits only; relevance, claims, and repetition need
evidence-led review.

## Counting boundary

Runtime counts Rust `char` values for fields documented in characters and UTF-8
bytes for Apple's keyword field. Grapheme clusters and store-side normalization
can differ. Store submission UI/API remains final authority.
