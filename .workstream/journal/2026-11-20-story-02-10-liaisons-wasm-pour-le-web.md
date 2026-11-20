---
type: Journal
title: "Story 02-10 : liaisons WASM pour le web"
status: draft
created: 2026-11-20
---

# Story 02-10 : liaisons WASM pour le web

Commit `313b61b`. `pnpm check` vert : 87 tests Rust, 27 tests web (dont 12 de contrat), 11 serveur, 2 protocole.

## Ce qui est fait
- **`tessel-wasm`** : une classe `Core` qui couvre identité, *KeyPackages*, groupes, messages, historique, médias et codes de sécurité. Les échanges sont des objets simples (serde-wasm-bindgen, seule nouvelle dépendance). Les erreurs sont des `TesselError` avec un code stable (`group:pending-proposals`, `store:unreadable`, `media:altered`…).
- **`@tessel/core-web`** :
  - `TesselCore`, l'interface asynchrone typée (`api.ts`), que la liaison mobile (02-13) devra offrir telle quelle ;
  - `CoreHost` fait tourner le cœur dans le worker et persiste dans IndexedDB, en une transaction, ce que chaque appel a changé **avant** de répondre (contrat de l'ADR-007), même quand l'appel échoue ; les appels passent un par un ;
  - `serve` et `RemoteCore` transportent les appels par messages entre la page et le worker ; `worker.ts` est le point d'entrée du worker (la 12-01 le lancera) ;
  - `outboxOf(core)` branche `OutboxSender` (02-07) sur le cœur.
- **Tests de contrat** : les mêmes scénarios tournent sur l'hôte appelé directement et à travers un `MessageChannel` (création de compte et redémarrage, échanges dans un groupe, édition, médias, codes de sécurité, message écrit hors ligne envoyé après redémarrage).

## Décisions prises en cours de route
- **La clé de compte est gardée sur l'appareil** : `Client::create_account` la range dans l'enregistrement client scellé (format 2 ; le format 1 n'a jamais été publié). C'est le critère de la 02-02 (« les clés privées restent dans la base locale chiffrée »), et la 03-04 en aura besoin pour certifier un nouvel appareil. Aucune fonction ne la rend.
- **La clé d'un média ne sort jamais du WASM.** La page reçoit un `secret` opaque : clé, empreinte et taille scellées en XChaCha20-Poly1305 par une clé tirée au démarrage du cœur et jamais persistée (`MediaSeal`, dans `tessel-core`). Un secret ne s'ouvre que dans le cœur qui l'a scellé ; après un redémarrage, l'historique en redonne de nouveaux.
- **Médias en un seul bloc** pour l'instant (`encryptMedia` et `decryptMedia` sur le fichier entier). Le chiffrement par flux existe dans le cœur (02-08) ; il sera exposé quand l'envoi de gros fichiers (vidéos) arrivera.

## Taille du `.wasm`
- Mesurée à chaque passage des tests (`wasm-size.test.ts`), avec un budget dans `packages/core-web/wasm-size.json`.
- Le `strip` des symboles en release la divise presque par deux : de 3,1 Mo (705 Ko en gzip) à **1 784 525 octets (581 Ko en gzip, 435 Ko en brotli)**.
- `opt-level = "z"` ne gagnerait que 45 Ko en gzip, au prix de la vitesse de la crypto : écarté. L'essentiel du poids vient d'OpenMLS.
- Budget : 1 950 000 octets bruts et 640 000 en gzip.
