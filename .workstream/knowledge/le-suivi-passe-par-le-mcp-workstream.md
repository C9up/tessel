---
type: Préférence
title: "Le suivi passe par le MCP Workstream"
description: "Tout le travail sur `"
tags: [workstream, outillage]
status: draft
created: 2026-11-20
---

# Le suivi passe par le MCP Workstream

Tout le travail sur `.workstream/` (stories, epics, critères, états, reviews, journal, mémoire) passe par les outils du serveur MCP Workstream, pas par des scripts Python, `sed` ou des écritures directes dans les fichiers. Consigne du propriétaire, le 20.11.2026 : « tu as le mcp tu peux tout faire avec ».

Comment l'appliquer : `create_story` / `create_epic` pour créer, `set_state` pour l'état, `add_review` pour les remarques, `append_journal` pour les décisions, `note_knowledge` pour la mémoire, `read_doc` / `search` / `list_work` pour lire. Si un outil manque pour une modification (par exemple ajouter un critère à une story existante), chercher d'abord dans `guide` avant toute autre voie.
