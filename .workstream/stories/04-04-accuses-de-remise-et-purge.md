---
type: Story
title: "Accusés de remise et purge"
description: "Une enveloppe disparaît du serveur dès que tous ses destinataires l'ont reçue."
tags: [product]
status: draft
state: todo
parent: epics/04-acheminement-des-messages.md
depends_on: [stories/04-03-livraison-en-temps-reel.md]
priority: high
created: 2026-11-20
---

# Accusés de remise et purge

Une enveloppe disparaît du serveur dès que tous ses destinataires l'ont reçue.

## Critères d'acceptation

- L'appareil acquitte ce qu'il a reçu ; l'enveloppe est effacée quand tous les appareils l'ont acquittée.
- Une enveloppe jamais acquittée est purgée après une durée de rétention configurable (tâche `bay`, durées `chronos`).

## Reviews

```workstream
reviews
```
