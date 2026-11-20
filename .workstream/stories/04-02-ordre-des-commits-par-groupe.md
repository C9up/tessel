---
type: Story
title: "Ordre des commits par groupe"
description: "Un seul commit gagne par epoch, pour que tous les membres restent d'accord."
tags: [product]
status: draft
state: todo
parent: epics/04-acheminement-des-messages.md
depends_on: [stories/04-01-boites-aux-lettres-des-appareils.md]
priority: high
created: 2026-11-20
---

# Ordre des commits par groupe

Un seul commit gagne par epoch, pour que tous les membres restent d'accord.

## Critères d'acceptation

- Un verrou par groupe (`eclipse`) n'accepte qu'un commit par epoch ; les autres reçoivent un refus nommé.
- Les messages reçoivent un numéro de séquence par groupe.
- L'auteur d'un commit reçoit « accepté » ou « refusé », jamais son propre commit : chiffré (`PrivateMessage`), il ne peut pas le relire (`commit_accepted` / `commit_rejected` du cœur, story 02-04).
- Test de concurrence : deux commits simultanés, un seul accepté, l'autre rejoué par le client.

## Reviews

```workstream
reviews
```
