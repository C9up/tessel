---
type: Journal
title: "Story 03-03 : annuaire des KeyPackages"
status: draft
created: 2026-11-21
---

# Story 03-03 : annuaire des KeyPackages

Commit dd8c7af. `pnpm check` vert : Rust 90 + 8 + 2, web 28, serveur 29 (7 nouveaux), protocole 7.

## Parcours
- **Dépôt** (`identity.publishKeyPackages`, session requise) : c'est ce que donne `refreshKeyPackages` : nouveaux, dernier recours, retraits. Tout passe ou rien. Le serveur vérifie chaque KeyPackage par le WASM du cœur : signatures, suite, validité, et même certificat que celui qu'il garde pour cet appareil (un appareil ne dépose que pour lui-même). Le bon type est aussi exigé : un dernier recours ne passe pas pour un usage unique.
- **Retrait** (`identity.claimKeyPackages {account}`) : un KeyPackage par appareil vivant du compte ; celui de l'appelant est exclu. Le serveur prend l'usage unique le plus proche de sa fin, le verrouille (`FOR UPDATE SKIP LOCKED`) et le supprime en une seule requête : jamais remis deux fois, même à des retraits simultanés. Si le stock est vide, le dernier recours sert et reste. Compte inconnu ou nom impossible : liste vide. Autre instance : `E_IDENTITY_WRONG_DOMAIN`.
- **Stock** (`identity.keyPackageStock`) : `{stock, low}`. Le dépôt renvoie la même chose.
- **Prévenir l'appareil** : l'événement `identity.keyPackages.low` `{deviceId, stockLeft}` part quand un retrait fait passer le stock sous 25, puis quand il le vide ; une seule fois par franchissement, pas à chaque retrait. Rien ne le relaie encore jusqu'à l'appareil : c'est le canal temps réel de l'epic 04. D'ici là, l'appareil voit `low` à chaque dépôt ou consultation.

## Choix faits seul
- **L'annuaire est dans le module identity** (l'epic 03 le place là), table `key_packages`.
- **Plafond de 200 usages uniques par appareil** (sa réserve en fait 100) et de 128 KeyPackages par envoi : sans cela, n'importe quel appareil pourrait remplir la base.
- **Limites** : 60 retraits par minute par appareil demandeur, de quoi créer un grand groupe sans vider le stock de tout le monde ; 10 dépôts par minute par appareil.
- **Révocation** : elle supprime aussi les KeyPackages de l'appareil, dans la même requête que ses sessions (`SqlDevices.revoke`).
- **Date de fin au jour près**. Un KeyPackage n'est plus remis le jour où sa validité se termine, donc jusqu'à un jour plus tôt. Le cœur expose maintenant `VerifiedKeyPackage::not_after()`, et le DTO `notAfter`.

## Registre des métadonnées (story 15-02)
- `key_packages.reference` : le `KeyPackageRef`, une empreinte.
- `key_packages.device_id` : l'appareil.
- `key_packages.key_package` : des clés publiques et le certificat de l'appareil, que les contacts voient de toute façon.
- `key_packages.last_resort` et `not_after` (date).
- Ce que le serveur peut en déduire : le rythme auquel le stock d'un appareil baisse, c'est-à-dire à peu près à quelle fréquence on l'ajoute à des groupes. Il ne sait ni qui l'ajoute ni à quel groupe : le retrait ne laisse aucune trace en base. Seuls restent le compteur blackhole de l'appareil demandeur, pendant une minute, et l'événement de stock bas.
- Événement `identity.keyPackages.low` : identifiant de l'appareil, stock restant, heure (`occurredAt`) ; il passe par l'outbox.
- Compteurs blackhole `identity_claim_<appareil>` et `identity_upload_<appareil>` : une minute.

## Tests (serveur, de bout en bout)
- Dépôt de la réserve (100), retrait d'un KeyPackage valide pour le bon appareil, stock à 99 ; ses propres appareils exclus ; compte inconnu → liste vide.
- 8 retraits simultanés sur un stock de 5 : 5 KeyPackages distincts, puis 3 fois le dernier recours.
- L'événement part une fois, au franchissement du seuil (24 restants), et pas au retrait suivant.
- Refus de tout l'envoi, sans rien garder : KeyPackage d'un autre appareil mêlé aux siens, dernier recours présenté comme usage unique, octets quelconques.
- Un KeyPackage retiré n'est plus remis.
- Un appareil révoqué n'est plus proposé.
- Sans session : `E_UNAUTHENTICATED`.
