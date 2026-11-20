---
type: Epic
title: "Médias"
description: "Photos, messages vocaux, fichiers et vidéos, chiffrés sur l'appareil avant de partir."
tags: [product]
status: draft
order: 6
created: 2026-11-20
---

# Médias

Photos, messages vocaux, fichiers et vidéos, chiffrés sur l'appareil avant de partir.

## Périmètre

- Module serveur `media` : blobs chiffrés stockés par `archive` (S3), expiration et purge.
- Traitement côté client : réduction, miniatures, retrait des métadonnées, forme d'onde.

## Terminé quand

- Une photo envoyée du web s'affiche sur Android et l'inverse.
- Le serveur ne connaît ni le type, ni le nom, ni le contenu d'un fichier.

## Dépendances

- Epics 02 (chiffrement des médias) et 05.

## Stories

```workstream
stories
```
