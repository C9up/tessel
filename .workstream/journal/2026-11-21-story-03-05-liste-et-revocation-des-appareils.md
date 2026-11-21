---
type: Journal
title: "Story 03-05 : liste et révocation des appareils"
status: draft
created: 2026-11-21
---

# Story 03-05 : liste et révocation des appareils

Commit 1f36314. `pnpm check` vert : Rust 99 + 8 + 2 (3 nouveaux), web 32, serveur 39 (5 nouveaux, stables sur 3 passages), protocole 9.

## Ce qui est fait
- **Liste** (`identity.devices`, session requise). Pour chaque appareil actif du compte :
  - son type (`web`, `android`, `ios`), déclaré à chaque connexion ; `signIn` prend maintenant `kind` ;
  - le jour de création et le dernier jour d'activité (`YYYY-MM-DD`, jamais l'heure) ;
  - `current` pour l'appareil qui demande.

  Le dernier jour d'activité est mis à jour à la connexion et par `touch` de la session, au plus une fois par jour, dans la même requête.
- **Révocation** (`identity.revokeDevice {device, revocation?}`) :
  - un appareil peut se révoquer lui-même sur sa seule session ;
  - pour en révoquer un autre, il faut la signature de la clé de compte : label « Tessel 1.0 DeviceRevocation » sur (version, compte, identifiant d'appareil). Le cœur la vérifie côté serveur avec la clé que le serveur garde, jamais une clé apportée par la demande.

  Sessions et KeyPackages tombent dans la même requête (`SqlDevices.revoke`). Événement `identity.device.revoked`.
- **Retrait des groupes**, dans le cœur :
  - `Client::groups_with(device)` donne les groupes à quitter ;
  - `remove_devices` produit les commits ;
  - un test Rust et le contrat web montrent la tablette retirée du groupe, et Bob qui traite le commit.
- **Contacts vérifiés avertis** :
  - un commit qui retire un appareil d'un contact vérifié le signale (`Incoming::Commit::verified_removed`, `verifiedRemoved` en TS) ;
  - l'appareil sort de la liste des appareils vérifiés : s'il revient, il est signalé comme nouveau (`Trust::NewDevice`), même après un redémarrage.

## Choix faits seul
- **La clé de compte est exigée pour révoquer un autre appareil.** Une tablette volée ne peut pas mettre Alice dehors de son téléphone. Le revers : sans l'appareil qui détient la clé, on ne révoque que soi-même ; la phrase de récupération couvrira ce cas.
- **Le type d'appareil est gardé en clair.** Le serveur l'apprend de toute façon par le service de notifications où l'appareil s'inscrit (epic 06 : FCM, APNs, Web Push). C'est dit dans la migration et au registre.
- **Un appareil retiré d'un groupe sort de la liste des appareils vérifiés, même s'il reste dans d'autres groupes.** C'est un cas rare (en pratique, on retire un appareil partout à la fois) ; au pire, il produit une alerte de trop, jamais une alerte manquée.
- **Le serveur ne fait pas les commits de retrait** : il ne peut pas (MLS). C'est l'appareil qui révoque qui les envoie. L'hôte les fera passer par l'acheminement de l'epic 04, comme pour la 03-04.

## Reste à brancher (epic 04)
Le transport des commits de retrait jusqu'aux autres membres. Les autres appareils du compte et les contacts pourront aussi réagir à `identity.device.revoked`, une fois relayé en temps réel.

## Registre des métadonnées (story 15-02)
- `devices.kind` : web, android ou ios.
- `devices.last_active_on` : le dernier jour d'activité.
- `devices.revoked_on` (déjà en 03-02).
- Événement `identity.device.revoked` : identifiant du compte, identifiant de l'appareil, heure.
- Compteur `identity_revoke_<appareil>` : une heure.
