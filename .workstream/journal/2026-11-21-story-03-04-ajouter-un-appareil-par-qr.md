---
type: Journal
title: "Story 03-04 : ajouter un appareil par QR"
status: draft
created: 2026-11-21
---

# Story 03-04 : ajouter un appareil par QR

Commit 9e671af. `pnpm check` vert : Rust 96 + 8 + 2 (6 nouveaux), web 32 (4 nouveaux, sur les deux transports), serveur 34 (5 nouveaux), protocole 8.

## Parcours
1. **Le nouvel appareil** (`requestLink`, `PendingLink` en WASM, `LinkRequest` dans le cœur) tire :
   - sa clé et son identifiant ;
   - un identifiant de liaison (128 bits) ;
   - un secret de 256 bits.

   Il les affiche dans un QR de 97 octets : version, liaison, secret, identifiant, clé publique. Sa clé privée reste dans le cœur.
2. **L'appareil qui détient la clé de compte** scanne (`grantLink`). Il certifie la clé du nouvel appareil et scelle le certificat en XChaCha20-Poly1305, avec une clé tirée du secret (HKDF-SHA256 : sel = identifiant de liaison, info = « Tessel 1.0 DeviceLink grant ») et la liaison en données associées. Il appelle ensuite `identity.linkDevice {certificate, link, grant}` avec sa session.
3. **Le serveur** vérifie le certificat (WASM du cœur), qu'il nomme le compte de l'appelant, et que sa clé de compte est celle du compte (`SqlDevices.add`). Il enregistre l'appareil et dépose la preuve scellée dans la boîte de la liaison : KeyDB `identity:link:<hex>`, `SET NX` pendant 5 minutes, donc seule la première est gardée. Il ne peut pas l'ouvrir. Événement `identity.device.added`.
4. **Le nouvel appareil** interroge `identity.takeLinkGrant {link}` (sans session ; `GETDEL`, remise une seule fois). Il ouvre la preuve et vérifie que le certificat certifie **sa** clé et **son** identifiant (`completeLink`), puis démarre comme appareil du compte : il peut se connecter (03-02) et déposer ses KeyPackages (03-03).
5. **Les groupes** : l'appareil qui a scanné prend un KeyPackage du nouvel appareil par groupe (`groups()`, `claimKeyPackages`) et l'ajoute (`addDevices`). Le nouvel appareil rejoint chaque groupe avec le *Welcome*.

## Où en sont les critères
- **Secret par QR puis canal éphémère chiffré** : fait. La boîte de la liaison est ce canal. Il ne sert que dans un sens (de l'appareil qui scanne vers le nouveau), ce qui suffit.
- **Certificat signé, puis tous les groupes du compte** :
  - fait dans le cœur : le test Rust montre le nouvel appareil dans les 2 groupes d'Alice, lisant un message de Bob ;
  - fait aussi dans le contrat web, sur les deux transports ;
  - **reste à brancher** : la boucle de l'hôte (pour chaque groupe : retirer un KeyPackage, ajouter, envoyer le commit au serveur). Le commit doit passer par le serveur pour être ordonné, et cet acheminement est l'epic 04. D'ici là, rien ne transporte les commits et les *Welcome* entre appareils réels.
- **Web → Android et Android → web** : **non fait**. La liaison mobile (02-13) est bloquée par atom 03-01. Le format du QR et le protocole ne dépendent pas de la plateforme ; il faudra seulement exposer `PendingLink` et `grantLink` dans la liaison mobile.

## Choix faits seul
- **Le nouvel appareil affiche le QR, l'ancien le scanne** (comme Signal). Le QR transporte la clé publique du nouvel appareil : l'ancien certifie exactement ce qu'il a vu.
- **La clé de compte ne bouge pas.** Seul l'appareil qui l'a peut lier un nouvel appareil. La perdre, c'est ne plus pouvoir ajouter d'appareil ; la phrase de récupération (décision du 20.11) couvrira ce cas.
- **Risque assumé** : quelqu'un qui filme le QR pourrait doubler l'appareil légitime avec une preuve de **son** compte. Le nouvel appareil rejoindrait alors le compte de l'attaquant, pas celui de la victime. La parade côté interface : afficher le compte rejoint (`DeviceInfo.account`) et le faire confirmer. C'est à faire dans le client web (epic 12).
- **Au plus 10 appareils actifs par compte.** Liaisons limitées à 10 par heure par appareil, et interrogations de la boîte à 60 par minute par réseau.
- **La boîte est d'abord réservée (`SET NX`), puis l'appareil enregistré.** Si l'enregistrement échoue, la boîte est vidée.

## Registre des métadonnées (story 15-02)
- `devices` : une ligne par appareil lié, avec les mêmes colonnes qu'en 03-01.
- KeyDB `identity:link:*` : une preuve scellée, illisible pour le serveur, pendant 5 minutes au plus.
- Événement `identity.device.added` : identifiant du compte, identifiant de l'appareil, heure.
- Compteurs `identity_link_<appareil>` (une heure) et `identity_grant_<réseau>` (une minute).
- Ce que le serveur apprend : qu'un compte a un appareil de plus, et quel jour.
