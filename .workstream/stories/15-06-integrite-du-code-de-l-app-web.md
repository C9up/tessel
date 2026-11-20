---
type: Story
title: "Intégrité du code de l'app web"
description: "Sur le web, le serveur sert le code à chaque chargement : un hébergeur malveillant pourrait servir une version qui vole les clés. Rendre ce code vérifiable et figé."
tags: [product]
status: draft
state: todo
parent: epics/15-securite-et-audit.md
depends_on: [stories/12-01-coquille-web-et-worker-crypto.md]
priority: high
created: 2026-11-20
---

# Intégrité du code de l'app web

Sur le web, le serveur sert le code à chaque chargement : un hébergeur malveillant pourrait servir une version qui vole les clés. Rendre ce code vérifiable et figé.

## Critères d'acceptation

- Étude consignée (ADR) : service worker qui fige le code installé et n'accepte une mise à jour que signée par la clé de publication ; extension de vérification façon Code Verify (WhatsApp) ; Isolated Web Apps et Web Bundles signés ; app de bureau, qu'on a écartée pour la v1.
- Build reproductible du client web et du WASM : n'importe qui reconstruit, à partir d'un commit, les mêmes empreintes que celles publiées.
- Empreintes de chaque version publiées hors du serveur qui sert l'app (dépôt public, journal de transparence), et Subresource Integrity sur chaque script et sur le WASM.
- CSP stricte et Trusted Types, aucun script tiers, médias servis sur une autre origine : une faille XSS ne doit pas atteindre le worker qui tient les clés.
- La limite restante est dite dans l'app et dans la doc, sans la cacher (principe 9 de la vision).

## Reviews

```workstream
reviews
```
