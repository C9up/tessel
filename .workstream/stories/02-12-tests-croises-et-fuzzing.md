---
type: Story
title: "Tests croisés et fuzzing"
description: "La preuve que le web et Android se comprennent, et que des entrées hostiles ne cassent rien."
tags: [product]
status: draft
state: doing
parent: epics/02-tessel-core-le-coeur-cryptographique.md
depends_on: [stories/02-10-liaisons-wasm-pour-le-web.md, stories/02-13-liaison-mobile-du-coeur-sur-la-stack-ream.md]
priority: medium
created: 2026-11-20
---

# Tests croisés et fuzzing

La preuve que le web et Android se comprennent, et que des entrées hostiles ne cassent rien.

## Critères d'acceptation

- Test d'interopérabilité WASM ↔ natif sur un groupe à plusieurs appareils.
- Vecteurs de test MLS officiels passés.
- `cargo fuzz` sur la désérialisation des messages et des médias entrants, intégré à `pnpm check` en courte durée.

## Reviews

```workstream
reviews
```
