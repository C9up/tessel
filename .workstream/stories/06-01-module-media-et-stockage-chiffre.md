---
type: Story
title: "Module media et stockage chiffré"
description: "Le serveur garde des blobs déjà chiffrés, sans savoir ce qu'ils sont."
tags: [product]
status: draft
state: todo
parent: epics/06-medias.md
depends_on: [stories/04-01-boites-aux-lettres-des-appareils.md, stories/02-08-chiffrement-des-medias.md]
priority: high
created: 2026-11-20
---

# Module media et stockage chiffré

Le serveur garde des blobs déjà chiffrés, sans savoir ce qu'ils sont.

## Critères d'acceptation

- Dépôt d'un blob chiffré via `archive` (S3), par une URL signée à durée courte.
- Identifiant opaque ; ni nom, ni type MIME stockés ; taille maximale.
- Le blob expire avec la rétention des messages.

## Reviews

```workstream
reviews
```
