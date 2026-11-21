---
type: Story
title: "Passer au WebSocket"
description: "Basculer le temps réel sur WebSocket quand relay le saura, SSE restant en repli."
tags: [product]
status: draft
state: blocked
parent: epics/04-acheminement-des-messages.md
depends_on: [stories/04-03-livraison-en-temps-reel.md]
priority: backlog
created: 2026-11-20
---

# Passer au WebSocket

Basculer le temps réel sur WebSocket quand relay le saura, SSE restant en repli.

## Critères d'acceptation

- Dépend de la story relay `ream-dev/packages/relay/.workstream/stories/01-05-transport-websocket-pour-relay.md`.
- Les clients ouvrent un WebSocket et retombent sur SSE en cas d'échec.
- Mesure avant / après : latence et consommation de batterie sur Android.

## Reviews

```workstream
reviews
```
