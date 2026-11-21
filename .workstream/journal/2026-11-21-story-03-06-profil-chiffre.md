---
type: Journal
title: "Story 03-06 : profil chiffré"
status: draft
created: 2026-11-21
---

# Story 03-06 : profil chiffré

Commit 29aa5a0. `pnpm check` vert : Rust 105 + 8 + 2 (6 nouveaux), web 34 (2 nouveaux), serveur 44 (5 nouveaux), protocole 10.

## Ce qui est fait
- **Clé de profil** :
  - 256 bits par compte, tirée à la création du compte ;
  - gardée dans l'enregistrement client scellé, dont le format passe en version 3 (la version 2 n'a jamais été publiée) ;
  - remise aux appareils liés dans le laissez-passer du QR, qui contient désormais le certificat puis la clé (03-04).
- **Profil** : un nom (1 à 128 octets) et un avatar facultatif (au plus 64 Kio, transporté dans le profil lui-même), complétés de zéros comme un message (Padmé).

  Scellement en XChaCha20-Poly1305, avec une clé tirée de la clé de profil (HKDF, « Tessel 1.0 profile »). Les données associées lient le blob à son compte : le serveur ne peut pas servir le profil de Bob comme celui d'Alice.
- **Transmission aux contacts par MLS** : `Body::ProfileKey` (type 6).
  - `Client::share_profile(group)` la place dans la file du groupe ;
  - le destinataire garde la clé du contact (enregistrement scellé `profile-key`) et reçoit `Effect::ProfileUpdated` ;
  - rien n'entre dans l'historique ;
  - la page ne voit jamais la clé : le WASM rend `{ type: 'profileKey' }` sans contenu.
- **Changer de profil prévient les contacts** : on scelle le nouveau profil, on le dépose (`identity.setProfile`), puis on partage de nouveau la clé dans chaque groupe. Chaque contact reçoit `profile-updated` et va rechercher le blob.
- **Serveur** : `identity.setProfile {blob}` et `identity.profile {account}`, session requise pour les deux. Il ne vérifie que l'octet de version et la taille (80 Kio au plus), puisqu'il ne peut rien lire d'autre. Table `profiles`, événement `identity.profile.updated`.

## Choix faits seul
- **L'avatar est dans le profil**, pas dans le stockage des médias (epic 05, pas encore fait). 64 Kio suffisent pour une petite image (WebP 256×256).
- **La clé de profil ne tourne pas encore.** La faire tourner quand on bloque un contact (comme Signal) attendra l'epic 09 (contacts).
- **Quand partager la clé** : à l'hôte de le faire dans chaque groupe après un changement de profil, et à l'arrivée d'un nouveau contact. Ces messages sortiront par le même acheminement que le reste (epic 04).

## Registre des métadonnées (story 15-02)
- `profiles.blob` : illisible pour le serveur, qui n'en connaît que la taille, arrondie par le remplissage.
- `profiles.updated_on` : le jour du dernier changement.
- Événement `identity.profile.updated` : identifiant du compte et heure.
- Compteurs `identity_profile_set_<appareil>` (une heure) et `identity_profile_get_<appareil>` (une minute).
- Ce que le serveur apprend en plus : qui va chercher le profil de qui, c'est-à-dire, à peu près, qui connaît qui. Il ne le garde nulle part, mais il le voit passer. L'expéditeur scellé (18-02) ne couvre pas ce cas ; à noter pour l'audit.
