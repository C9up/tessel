---
type: Review
title: "Le critère mobile est retiré, et repris par 02-13 et 13-10"
description: "Le critère mobile est retiré, et repris par 02-13 et 13-10"
tags: [review]
status: draft
state: done
parent: stories/01-04-squelette-de-tessel-core-et-chaine-de-compilatio.md
review: suggestion
created: 2026-11-20
---

# Le critère mobile est retiré, et repris par 02-13 et 13-10

Le critère mobile est retiré, et repris par 02-13 et 13-10

Le critère « `version()` appelée depuis `apps/mobile` (module natif) » a été rempli le 20.11.2026, par uniffi, un module Expo local et un APK release sur émulateur (« tessel-core 0.1.0 » à l'écran, commit ea78b86). Il a été retiré le même jour : le propriétaire a fait de Tessel l'app pilote de la stack mobile de Ream (ADR-009 de Ream, epic 73), sans React Native ni Expo.

- App Expo, `tessel-ffi` et `uniffi-bindgen` sont supprimés ; ADR-003 est révisée.
- La preuve mobile revient avec la liaison de la stack Ream : stories 02-13 (liaison mobile du cœur) et 13-10 (coquille mobile).
- La partie web de 01-04 (`tessel-wasm`, `packages/core-web`, test Node) reste valable.

À valider : 01-04 sans son critère mobile, ou en attente de 02-13.

## Correction

```workstream
commits
```

- 
