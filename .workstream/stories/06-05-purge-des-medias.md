---
type: Story
title: "Purge des médias"
description: "Un média que plus personne ne peut lire disparaît du serveur."
tags: [product]
status: draft
state: todo
parent: epics/06-medias.md
depends_on: [stories/06-01-module-media-et-stockage-chiffre.md]
priority: medium
created: 2026-11-20
---

# Purge des médias

Un média que plus personne ne peut lire disparaît du serveur.

## Critères d'acceptation

- Un blob est effacé quand tous les destinataires l'ont téléchargé, ou à expiration.
- Tâche `bay` de purge ; test qui vérifie qu'aucun blob orphelin ne reste.

## Reviews

```workstream
reviews
```
