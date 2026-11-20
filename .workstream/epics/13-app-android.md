---
type: Epic
title: "App mobile sur la stack Ream"
description: "Tessel sur Android, puis iOS, par la stack mobile de Ream (sans React ni Expo) : les mêmes écrans aurora que le web, en vues natives. Tessel en est l'app pilote."
tags: [product]
status: draft
order: 13
created: 2026-11-20
---

# App mobile sur la stack Ream

Tessel sur Android, puis iOS, par la stack mobile de Ream (sans React ni Expo) : les mêmes écrans aurora que le web, en vues natives. Tessel en est l'app pilote.

## Périmètre

- Coquille mobile sur la stack mobile de Ream (ADR-009 de Ream, epic 73), liaison mobile de `tessel-core`, base locale chiffrée.
- Les écrans de l'epic 12, écrits une fois en aurora / nebula, rendus en vues natives : pas d'écran propre au mobile.
- Capacités natives (caméra, galerie, QR, carnet d'adresses) et notifications FCM.
- Ce qui manque à la stack devient du travail pour Ream.
- Les stories de la première version de cette epic (Expo) sont archivées dans `archive/stories/` (20.11.2026).
- Distribution aux proches (builds et signature par ream-cli).

## Terminé quand

- Le parcours de la première étape réussie se fait entièrement sur Android.
- Les proches installent l'app sans passer par une procédure technique.
- Aucune dépendance à React ou à Expo.

## Dépendances

- Epics 02 à 12 (les écrans viennent de l'epic 12).
- La stack mobile de Ream : `ream:epics/73-mobile-apps-on-the-ream-ecosystem-without-react.md`.

## Stories

```workstream
stories
```
