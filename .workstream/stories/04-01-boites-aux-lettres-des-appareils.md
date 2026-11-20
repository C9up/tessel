---
type: Story
title: "Boîtes aux lettres des appareils"
description: "Chaque message est rangé une fois et livré à chaque appareil membre du groupe."
tags: [product]
status: draft
state: todo
parent: epics/04-acheminement-des-messages.md
depends_on: [stories/03-02-authentification-par-la-cle-d-appareil.md]
priority: high
created: 2026-11-20
---

# Boîtes aux lettres des appareils

Chaque message est rangé une fois et livré à chaque appareil membre du groupe.

## Critères d'acceptation

- Un message de groupe est stocké une fois, avec la liste des appareils qui doivent le recevoir.
- Le contenu est un blob opaque ; seul l'en-tête public MLS est lu.
- Un appareil ne lit que ce qui lui est destiné.

## Reviews

```workstream
reviews
```
