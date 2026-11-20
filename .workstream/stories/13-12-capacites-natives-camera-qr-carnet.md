---
type: Story
title: "Capacités natives : caméra, galerie, QR et carnet d'adresses"
description: "Ce que seul le téléphone offre, par les capacités natives de la stack Ream (des crates Rust, pas des modules par plateforme)."
tags: [product]
status: draft
state: todo
parent: epics/13-app-android.md
depends_on: [stories/13-10-coquille-mobile-sur-la-stack-ream.md]
priority: high
created: 2026-11-20
---

# Capacités natives : caméra, galerie, QR et carnet d'adresses

Ce que seul le téléphone offre, par les capacités natives de la stack Ream (des crates Rust, pas des modules par plateforme).

## Critères d'acceptation

- Caméra et galerie pour les photos ; métadonnées retirées avant chiffrement (vérifié sur une photo géolocalisée).
- Lecture d'un QR à la caméra (ajout d'appareil, code de sécurité, contact).
- Lecture du carnet d'adresses, demandée au bon moment, pour la recherche privée de contacts (09-03) ; rien ne part en clair.
- Une capacité absente de la stack est un ticket pour Ream, écrite en Rust selon son ADR-009.

## Reviews

```workstream
reviews
```
