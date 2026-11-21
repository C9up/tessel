---
type: Journal
title: "Story 03-02 : authentification par la clé d'appareil"
status: draft
created: 2026-11-21
---

# Story 03-02 : authentification par la clé d'appareil

Commit 216d975. `pnpm check` vert : Rust 90 + 8 + 2, web 28, serveur 22 (6 nouveaux), protocole 6.

## Parcours
1. `identity.challenge` : le serveur tire 32 octets aléatoires et les garde une minute dans KeyDB (`identity:challenge:<hex>`). La clé ne nomme ni appareil ni réseau.
2. L'appareil signe avec sa clé (`Core.signIn`, dans `tessel-core`). Contenu signé : `SignContent` avec le label « Tessel 1.0 SignIn » et la structure TLS (version, défi, compte, identifiant d'appareil). Le label diffère des labels MLS et de celui du certificat : une preuve ne vaut rien ailleurs.
3. `identity.signIn {device, challenge, proof}` : le défi est consommé d'abord (un `DEL` à 1, donc usage unique, même en cas d'échec). Ensuite l'appareil est lu en base (non révoqué), puis la preuve est vérifiée par le WASM du cœur (`verifySignIn`) avec le certificat que le serveur garde.
4. En cas de succès : jeton d'accès warden `tsn_<id>.<secret>`, d'une durée de 30 jours. Le propriétaire du jeton est l'appareil (son identifiant en hex), pas le compte.
5. `identity.session` (qui suis-je) et `identity.signOut` (ferme cette session-là seulement). `sessionGuard(deps)` est exporté pour les points des autres modules.

## Choix faits seul
- **Un seul refus pour tout échec** (`E_IDENTITY_SIGN_IN_REFUSED`) : défi périmé, appareil inconnu ou révoqué, signature fausse. Rien n'indique quels appareils existent.
- **Limites par réseau, pas par appareil.** Défis : 30 par minute ; connexions : 10 par minute. Une limite par appareil permettrait à quiconque connaît l'identifiant (les contacts le voient dans le certificat) de bloquer l'appareil. Une signature Ed25519 ne se devine pas : la limite borne seulement le travail imposé au serveur.
- **Révocation** : la colonne `devices.revoked_on` est posée dès maintenant. `SqlDevices.revoke` marque l'appareil et supprime ses sessions dans une seule requête. En plus, `Authenticate` relit l'appareil à chaque appel, donc une session est refusée dès la révocation. Le point RPC de révocation et l'interface viennent en 03-05.
- **Le jeton passe dans `Authorization: Bearer`, jamais dans le corps.** `fetchTransport(baseUrl, { fetch, token })` : la signature change (avant : `fetchImpl` en second argument). Elle n'avait aucun appelant.
- **Dates au jour près**, comme en 03-01 (ADR-006) : `created_on`, `last_used_on`, `expires_on`. Conséquence : une session expire au début du jour où ses 30 jours se terminent, jusqu'à un jour plus tôt. `touch` n'écrit qu'une fois par jour.
- **pnpm** a ajouté `minimumReleaseAgeExclude: '@c9up/warden@0.2.7'` dans `pnpm-workspace.yaml` : la version est plus récente que le délai de sécurité de pnpm. C'est un paquet @c9up, je l'ai gardé ; à retirer quand le délai sera passé.

## Registre des métadonnées (story 15-02)
- `devices.revoked_on` (date) : le jour où l'appareil a été révoqué.
- `sessions.id` : identifiant aléatoire de 128 bits, la moitié publique du jeton.
- `sessions.device_id` : l'appareil de la session.
- `sessions.hash` : sha256 du secret ; une copie de la table n'ouvre aucune session.
- `sessions.created_on`, `last_used_on`, `expires_on` : au jour près. `last_used_on` dit le dernier jour d'activité de l'appareil : c'est la métadonnée la plus sensible de la table.
- KeyDB `identity:challenge:*` : une valeur aléatoire pendant 60 s, rien d'autre.
- Compteurs blackhole `identity_challenge_<réseau>` et `identity_sign_in_<réseau>` : le réseau (IPv4, ou /64 en IPv6), pendant la fenêtre d'une minute.

## Tests
- Rust, 3 nouveaux. Une preuve ne vaut que pour son défi et son appareil. Une signature MLS brute du défi n'est pas une preuve. Un défi de mauvaise taille est refusé.
- Web : `signIn` renvoie 64 octets, et `identity:malformed-challenge` sort sur un défi de 31 octets.
- Serveur (de bout en bout), 6 tests :
  - une session est liée à l'appareil ;
  - la base ne garde que le hash du jeton ;
  - un rejeu, la preuve d'un autre appareil, un défi inventé ou un appareil inconnu reçoivent le même refus ;
  - sans jeton ou avec un jeton forgé, la session est refusée ;
  - la déconnexion ne ferme qu'une session ;
  - après révocation, la session est refusée, plus aucune ligne ne reste en base, et la reconnexion est refusée ;
  - au 11e essai dans la minute : `E_TOO_MANY_REQUESTS`.
