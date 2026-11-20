---
type: Story
title: "Revue de sécurité interne"
description: "Ce qu'on peut vérifier soi-même avant de payer un audit."
tags: [product]
status: draft
state: todo
parent: epics/15-securite-et-audit.md
depends_on: [stories/15-03-test-serveur-aveugle.md]
priority: medium
created: 2026-11-20
---

# Revue de sécurité interne

Ce qu'on peut vérifier soi-même avant de payer un audit.

## Critères d'acceptation

- CSP, en-têtes, dépendances (`cargo audit`, `cargo deny`, `pnpm audit`).
- Liste de contrôle OWASP MASVS pour l'app Android, ASVS pour le serveur.
- Constats corrigés ou écartés avec leur raison.

## Reviews

```workstream
reviews
```
