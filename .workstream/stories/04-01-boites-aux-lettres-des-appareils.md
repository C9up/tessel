---
type: Story
title: "Boîtes aux lettres des appareils"
description: "Chaque message est rangé une fois et livré à chaque appareil membre du groupe."
tags: [product]
status: draft
state: review
parent: epics/04-acheminement-des-messages.md
depends_on: [stories/03-02-authentification-par-la-cle-d-appareil.md]
priority: high
created: 2026-11-20
commits: [2dde26f4f1b4e2c7934ddbe6fd4b663020a78cc8]
---

# Boîtes aux lettres des appareils

Chaque message est rangé une fois et livré à chaque appareil membre du groupe.

## Critères d'acceptation

- Un message de groupe est stocké une fois, avec la liste des appareils qui doivent le recevoir.
- Le contenu est un blob opaque ; seul l'en-tête public MLS est lu.
- Un appareil ne lit que ce qui lui est destiné.
- Un message renvoyé avec le même identifiant d'envoi (`SendId`, 16 octets tirés au hasard, 02-07) par le même appareil n'est rangé qu'une fois, et l'accusé de réception est rendu de nouveau. La clé de déduplication est le couple (appareil expéditeur, identifiant d'envoi) ; elle est gardée le temps de la purge (04-04) et inscrite au registre des métadonnées (15-02).

## Reviews

```workstream
reviews
```
