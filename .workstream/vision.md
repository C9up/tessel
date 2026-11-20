---
type: Vision
title: "Tessel — vision"
description: "Messagerie chiffrée de bout en bout, open source, web et mobile, qui remplace WhatsApp et emprunte à Discord ses communautés, bâtie sur c9up Ream."
tags: [product]
status: draft
created: 2026-11-15
---

# Tessel — vision

Messagerie chiffrée de bout en bout, open source, web et mobile, qui remplace WhatsApp et emprunte à Discord ses communautés, bâtie sur c9up Ream.

> Version 1 (16.11.2026). Les choix viennent du cadrage avec le propriétaire, consigné dans `journal/2026-11-15-cadrage-de-la-vision.md` et `journal/2026-11-16-cadrage-de-la-vision.md`. Révisée le 20.11.2026 : BlackGate retiré (ADR-006), limite de l'app web (story 15-06).

## Pourquoi

WhatsApp est devenu l'infrastructure de nos conversations, mais il appartient à Meta. Il demande un numéro de téléphone, collecte les métadonnées (qui parle à qui, quand, d'où) et ne propose que des groupes plats, qui s'effondrent dès qu'une communauté grandit.

Discord a résolu la structure des communautés : espaces, salons, rôles, fils, salons vocaux où l'on entre et sort. Mais ses messages texte ne sont pas chiffrés de bout en bout, et il vit de ce qu'il sait de ses utilisateurs.

Tessel veut réunir l'intimité de Signal, la structure de Discord et la simplicité de WhatsApp, sur le web comme sur mobile. Il est **open source** et pensé pour être **auto-hébergé**.

**Pour qui.** D'abord les proches du propriétaire : famille et amis. Ils doivent pouvoir quitter WhatsApp sans effort. Un produit public viendra ensuite, mais seulement après un audit de sécurité externe.

**Le nom.** Une *tessella* est la petite tesselle d'une mosaïque. Chaque conversation est une tesselle, scellée et opaque. Assemblées, elles forment la mosaïque d'une communauté, et personne d'autre que ses membres ne voit le dessin. Le nom est définitif.

## Principes

1. **Le serveur ne lit rien.** Le chiffrement de bout en bout est la règle, par défaut, partout, y compris dans les espaces publics. Le serveur est un facteur aveugle : il authentifie, stocke et achemine des enveloppes qu'il ne sait pas ouvrir. Une fonction qui exigerait que le serveur lise un contenu (recherche, aperçu de lien, miniature) se fait sur l'appareil, ou ne se fait pas. Sa base ne garde que du chiffré de bout en bout, des identifiants opaques, et ce qu'il doit comparer ou trier (ADR-006).
2. **Le moins de métadonnées possible.** Le numéro de téléphone est facultatif. La recherche de contacts se fait en privé : un simple hachage des numéros ne suffit pas, on utilise une recherche privée à débit limité (OPRF) et le serveur ne garde aucun carnet. L'expéditeur est scellé quand c'est possible. Les notifications push ne portent aucun contenu. Messages et journaux expirent. Chaque donnée que le serveur connaît est inscrite, avec sa raison, au registre des métadonnées.
3. **Pas de cryptographie maison.** On s'appuie sur MLS (*Messaging Layer Security*, RFC 9420) via OpenMLS, et sur des primitives standard. Tessel assemble, il n'invente pas.
4. **Un cœur client unique, écrit en Rust.** Le protocole, la base locale chiffrée, la file d'envoi, le chiffrement des médias et les sauvegardes vivent dans un seul crate, `tessel-core`. Sur le modèle de Ream (« un cœur Rust, une liaison par hôte ») : WASM pour le web, la liaison mobile de la stack Ream (ABI C + JSI) pour le téléphone. On l'écrit une fois et on l'audite une fois. Il reste propre à Tessel tant qu'aucune autre app n'en a besoin.
5. **Le vrai multi-appareil.** Chaque appareil est un membre à part entière d'un groupe MLS. Un nouvel appareil reçoit l'historique, chiffré, transféré par un appareil déjà connecté (QR). Retirer un appareil volé révoque ses clés.
6. **On ne perd pas tout avec son téléphone.** Une sauvegarde chiffrée par une phrase de récupération que seul l'utilisateur connaît est stockée sur le serveur, qui ne peut pas la lire.
7. **Ream, à la Ream, et rien de propriétaire.** Le serveur est une app Ream en modules hexagonaux sur le modèle `nadeo` (`domain / application / infrastructure / http`, test d'architecture, outbox, bus d'événements `@c9up/ream/events`). Il utilise l'écosystème `@c9up` au maximum. Ce qui manque à Ream devient du travail pour Ream, pas une rustine dans Tessel. Tessel ne dépend d'aucun produit propriétaire : quiconque clone le dépôt peut le lancer, le tester et l'héberger (ADR-006).
8. **Fédérable par construction.** Chaque identité porte son serveur (`alice@tessel.exemple`) dès le premier jour, et le protocole suit la piste IETF MIMI, bâtie sur MLS. Les serveurs ne se parlent qu'après la v1, mais rien n'est construit qui l'empêcherait.
9. **Honnête sur les limites.** Le chiffrement de bout en bout borne la taille d'un espace à quelques milliers de membres. La modération ne peut se faire que sur signalement, la recherche que sur l'appareil. Sur le web, c'est le serveur qui sert le code de l'app à chaque chargement : un hébergeur malveillant pourrait servir une version piégée. Tessel rend ce code vérifiable (story 15-06) et dit la limite qui reste. Tessel le dit et choisit ; il ne fait pas semblant.

## Menace visée

En priorité, **l'hébergeur et les fuites** : l'opérateur, lui compris, ne peut rien lire, et une base volée ne révèle aucun contenu. Sur le web, la menace de l'hébergeur passe aussi par le code qu'il sert (story 15-06). Le téléphone saisi (verrou de l'app, effacement à distance) et la surveillance d'État (masquer le graphe social au serveur, se passer de Google et Apple) ne sont pas les objectifs de la v1. Ils guident les choix pour ne pas les fermer.

## Ce que Tessel emprunte

| À qui | Quoi | Quand |
|---|---|---|
| WhatsApp | Conversations à deux et petits groupes, photos, messages vocaux, accusés de lecture, prise en main immédiate | v1 |
| Discord | **Espaces** faits de **salons**, **rôles et permissions** | v1 |
| Discord | Fils, salons vocaux permanents, réactions, épingles, présence | après la v1 |
| WhatsApp / Discord | Appels audio et vidéo, à deux puis en groupe | après la v1 |
| Signal | Vérification de l'interlocuteur (code de sécurité, QR), expéditeur scellé, messages éphémères | v1 pour la vérification, le reste ensuite |
| Signal, Telegram, Discord | Bots, membres visibles des salons, ajoutés par les admins (epic 22) | après la v1 |

**Espaces publics.** Un espace peut s'ouvrir par un simple lien, sans invitation personnelle. Il reste chiffré de bout en bout.

**Modération.** Chaque espace se modère lui-même : ses admins excluent, suppriment et bannissent. L'opérateur reçoit les signalements, où le message est transmis par celui qui signale. Il peut bloquer un compte ou fermer un espace entier, mais jamais lire ce qu'on ne lui a pas montré.

## Architecture

**Clients**
- **Une seule interface, écrite en aurora et nebula**, rendue dans le navigateur aujourd'hui et en vues natives sur le téléphone demain, par la stack mobile de Ream (son ADR-009, epic 73) : **ni React Native, ni Expo**. Tessel est l'**app pilote** de cette stack : il la met à l'épreuve, et ce qui lui manque devient du travail pour Ream.
- **Web** : le serveur ne sert que la coquille ; le déchiffrement se fait dans le navigateur, avec `tessel-core` en WASM dans un Web Worker et un service worker pour Web Push. Le code servi est vérifiable : build reproductible, empreintes publiées hors du serveur (story 15-06). Pas d'app de bureau en v1.
- **Mobile** : Android d'abord, iOS ensuite, au rythme de l'epic 73 de Ream.

**Serveur Ream**, un module par domaine :
- `identity` : comptes, numéro facultatif, appareils, *KeyPackages* MLS, liens d'invitation, recherche privée de contacts.
- `delivery` : boîtes aux lettres d'enveloppes chiffrées, ordre des messages par groupe, accusés de remise, expiration.
- `spaces` : espaces, salons, rôles, liens publics. Le serveur connaît la structure, jamais les noms ni le contenu.
- `media` : blobs chiffrés par le client, stockés via `archive` (S3). Les miniatures se calculent côté client.
- `backup` : sauvegardes chiffrées par la phrase de récupération.
- `moderation` : signalements, blocage de comptes, fermeture d'espaces.
- `push` : réveil des appareils sans contenu.
- Après la v1 : `calls` (signalisation WebRTC, SFU, médias chiffrés par SFrame) et `federation`.

**Base de données** : PostgreSQL en TLS mutuel (`sslmode=verify-full`, certificat client), en dev comme en prod, sur un disque chiffré. Elle ne garde que du chiffré ou de l'opaque (ADR-006).

**Contrat client-serveur** : `comet` (JSON-RPC, client isomorphe pour le web et le mobile ; ce sera aussi la couche de données du mobile, avec les objets de page de photon (ADR-009 de Ream)). Le temps réel passe par `relay` : SSE pour recevoir, POST pour envoyer.

**Paquets @c9up** : `warden`, `sigil`, `rune`, `blackhole` (limitation de débit, anti-abus), `relay`, `quasar` (diffusion multi-instance), `bay` (push, expirations), `eclipse` (une seule *epoch* MLS à la fois par groupe), `archive`, `nova`, `comet`, `chronos`, `rosetta`, `spectrum`, `parsec`, `helix`, `atlas`.

**Hébergement** : Outpost, sur les VPS du propriétaire. Une instance doit aussi s'installer facilement chez quiconque.

## Travail pour Ream

Ces chantiers se mènent dans Ream, pas en contournement dans Tessel :
- **WebSocket dans `relay`**, via un *upgrade* dans le serveur HTTP Rust. Tessel démarre en SSE ; ce chantier doit aboutir avant les appels.
- **Push natif dans `nova`**, avec des pilotes FCM et APNs à côté de Web Push (prévu par l'ADR-009 de Ream ; epic nova 03). UnifiedPush viendra ensuite, pour les Android sans Google.
- **La stack mobile (epic 73 de Ream)** : hôte natif, rendu des composants aurora en vues natives, liaison mobile des cœurs Rust, builds et signature par ream-cli. Tessel en est l'app pilote.
- **`prism` ne voit pas les médias chiffrés.** Le traitement d'image de Tessel se fait côté client ; rien à changer dans prism, mais à savoir.

## Hors objectifs

- Publicité, revente de données, IA qui lit les conversations.
- Tout mode « non chiffré », même pour les grands espaces.
- Passerelle côté serveur vers WhatsApp ou Matrix. Une passerelle **sur l'appareil**, en option, pourra venir plus tard pour faciliter la transition.
- Toute dépendance à un produit propriétaire.
- Échéance : il n'y en a pas, le projet avance à son rythme.

## Première étape réussie

Deux personnes, l'une sur le web et l'autre sur Android (stack mobile de Ream), échangent du texte et des photos chiffrés de bout en bout. Elles rejoignent un groupe de quelques membres. Chacune ajoute un second appareil, qui reçoit l'historique. Elles reçoivent une notification push sans contenu et vérifient leurs codes de sécurité. Le serveur, inspecté, ne contient aucun message lisible.

## Avant le public

- Audit de sécurité externe de `tessel-core` et du protocole.
- Intégrité vérifiable du code de l'app web (story 15-06).
- Domaine réservé, homonymes vérifiés.
