---
type: Journal
title: "Story 03-01 : module identity et création de compte"
status: draft
created: 2026-11-21
---

# Story 03-01 : module identity et création de compte

Commit `b8d51fc`. `pnpm check` vert : serveur 16 tests (5 nouveaux), web 28, protocole 4, Rust 87 + 8 + 2.

## Le parcours
1. L'appareil crée sa clé de compte et sa clé d'appareil dans le cœur (`TesselCore.createAccount`, 02-10).
2. Il envoie au serveur le certificat que la première signe pour la seconde (`identity.createAccount`, en base64url).
3. Le serveur vérifie :
   - la signature, avec le module WASM de `tessel-core` chargé dans Node (`@tessel/core-web/server`) : aucune cryptographie écrite en TypeScript ;
   - que le compte est `pseudo@APP_DOMAIN` et que le pseudo suit l'ADR-002 ;
   - que le pseudo est libre.
4. Il garde le compte et son premier appareil dans une seule requête : pas de compte sans appareil.
5. Il publie `AccountCreated` (`identity.account.created`). L'événement ne nomme que l'identifiant opaque du compte.

## Choix faits
- **Rien d'autre n'est demandé** : ni e-mail, ni numéro, ni nom affiché.
- **Limite de créations** : 5 par heure et par réseau (une adresse IPv4, ou un /64 en IPv6), avec le limiteur de blackhole. L'adresse ne sert que de clé de comptage, le temps de la fenêtre ; elle n'est écrite nulle part.
- **Erreurs** : le code JSON-RPC, plus un nom stable dans le message (`E_IDENTITY_NAME_TAKEN`, `E_IDENTITY_WRONG_DOMAIN`, `E_IDENTITY_INVALID_CERTIFICATE`, `E_TOO_MANY_REQUESTS`, `E_INVALID_PARAMS`), déclarés une fois dans `@tessel/protocol` (`ServerError`). Le client les traduit.
- **`APP_DOMAIN`** est une nouvelle variable d'environnement, présente dans `.env.example` (donc dans la CI).
- **Pas fait ici** : le refus de démarrer quand `APP_DOMAIN` change (ADR-002, conséquences). C'est à prévoir avec l'hébergement (epic 14).

## Pour le registre des métadonnées (15-02)
| Table | Colonne | Ce que le serveur sait | Pourquoi | Durée |
|---|---|---|---|---|
| `accounts` | `id` | identifiant opaque (uuid aléatoire) | clé, et seul nom du compte dans les événements | vie du compte |
| `accounts` | `name` | le pseudo | le rendre unique, le retrouver (annuaire, 03-03) | vie du compte |
| `accounts` | `account_key` | la clé publique du compte | vérifier les certificats de ses appareils | vie du compte |
| `accounts` | `created_on` | le jour de création, pas l'heure | ce que Signal répond à une réquisition, rien de plus | vie du compte |
| `devices` | `id` | identifiant aléatoire de 128 bits tiré par l'appareil | clé | vie de l'appareil |
| `devices` | `account_id` | à quel compte est l'appareil | lister les appareils d'un compte | vie de l'appareil |
| `devices` | `device_key` | la clé publique de l'appareil | authentifier l'appareil (03-02) | vie de l'appareil |
| `devices` | `certificate` | le certificat signé | le remettre aux contacts (03-03) | vie de l'appareil |
| `devices` | `created_on` | le jour d'ajout | — | vie de l'appareil |
| limiteur (mémoire ou KeyDB) | clé `identity_create_<réseau>` | qu'un réseau a créé N comptes | limiter les créations | une heure |
