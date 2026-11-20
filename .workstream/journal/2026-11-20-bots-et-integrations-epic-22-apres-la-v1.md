---
type: Journal
title: "Bots et intégrations : epic 22, après la v1"
status: draft
created: 2026-11-20
---

# Bots et intégrations : epic 22, après la v1

Question du propriétaire : « on pourra prévoir un système de bot comme sur Signal ? »

- Signal n'a pas d'API de bots officielle : un bot y est un compte ordinaire piloté par un programme (`signal-cli`). C'est le seul modèle compatible avec le chiffrement de bout en bout, et celui de Tessel.
- Un bot est un **membre du groupe MLS**, avec ses clés. MLS n'a pas de membre qui écrive sans lire : un bot lit tout ce que son salon lui montre. D'où les règles :
  - il est visible de tous ;
  - seul un admin l'ajoute, salon par salon ;
  - un rôle borne ce qu'il peut faire.
- Choix du propriétaire, 20.11.2026 :
  - **epic après la v1** (epic 22, stories en backlog) ;
  - **l'opérateur d'une instance peut proposer ses propres bots**, toujours marqués « bot de l'opérateur » et ajoutés seulement par un admin du salon.
- Écarté : des bots qui tournent dans le serveur et lisent sans être membres. Ce serait contraire à « le serveur ne lit rien ».
