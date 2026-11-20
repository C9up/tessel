---
type: Journal
title: "Cadrage de la vision"
status: draft
created: 2026-11-15
---

# Cadrage de la vision

Réponses du propriétaire aux questions de cadrage (15.11.2026), posées par petits lots.

**Lot 1 — public et modèle**
- Public : les proches d'abord (famille, amis), avec l'ambition d'un produit public plus tard.
- Modèle : open source.
- Inscription : numéro de téléphone **facultatif** (sans numéro par défaut, on peut en ajouter un pour être retrouvé).

**Lot 2 — comptes et appareils**
- Contacts : retrouver ses contacts depuis le carnet d'adresses, **en privé** (numéros hachés ou recherche privée, le serveur ne garde pas le carnet).
- Perte du téléphone : **sauvegarde chiffrée** par une phrase de récupération que seul l'utilisateur connaît ; le serveur la stocke sans pouvoir la lire.
- Nouvel appareil : l'historique est **transféré** chiffré par un appareil déjà connecté (QR).

**Lot 3 — fonctions**
- Discord en v1 : **espaces et salons**, **rôles et permissions**. Fils et salons vocaux permanents : plus tard.
- Appels audio et vidéo : **après la v1**.
- Espaces publics (rejoints par lien) : **oui, chiffrés quand même**, taille limitée à quelques milliers de membres.

## Suite

**Lot 4 — sécurité**
- Menace prioritaire : **l'hébergeur et les fuites**. Le serveur (opérateur compris) ne lit rien, et une fuite de la base ne révèle rien. Le téléphone saisi et la surveillance d'État ne sont pas visés en v1.
- Modération : **admins de l'espace + opérateur**. L'opérateur reçoit les signalements (message transmis par celui qui signale) et peut fermer un espace entier.
- Audit de sécurité externe : **oui, avant toute ouverture au-delà des proches**.

**Lot 5 — pile technique**
- Mobile : **Expo / React Native** (comme syndic et liste_course), avec le cœur Rust via un module natif.
- Bureau : **web seulement** ; une coquille Tauri plus tard si besoin.
- Temps réel : **SSE + POST d'abord** ; story dans Ream pour le WebSocket, avant les appels.
- `tessel-core` : **propre à Tessel** ; extrait en paquet @c9up quand une deuxième app en aura besoin.
- Push natif : **étendre `nova`** (Web Push, FCM, APNs par pilotes) ; UnifiedPush **plus tard**.
- Contrat d'API : **`comet`** (JSON-RPC, client isomorphe pour le web et Expo).
