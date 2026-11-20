---
type: Journal
title: "Story 02-08 : chiffrement des médias"
status: draft
created: 2026-11-20
---

# Story 02-08 : chiffrement des médias

**Choix**

- **Construction standard, aucune crypto maison** : STREAM (Hoang, Reyhanitabar, Rogaway, Vizár, 2015) sur ChaCha20-Poly1305, implémentée par RustCrypto (`aead::stream`, compteur 32 bits gros-boutiste). C'est la construction d'`age`. STREAM authentifie chaque bloc et sa position, et marque le dernier : bloc modifié, retiré, déplacé ou ajouté, ou blob coupé, tout échoue.
- **Dépendances** : `chacha20poly1305` (fonctionnalité `stream`) et `sha2` en dépendances directes. Les deux crates sont déjà dans l'arbre via le fournisseur RustCrypto d'OpenMLS : aucun nouveau code cryptographique n'entre.
- **Format du blob** : `[MEDIA_FORMAT=1][préfixe de nonce aléatoire, 7 octets][blocs de 64 Kio + étiquette de 16 octets]`. L'en-tête est la donnée associée de chaque bloc. Taille chiffrée calculable à l'avance (`encrypted_len`), pour les quotas (06-01).
- **Clé** : 32 octets aléatoires par fichier, jamais lisible hors du cœur. `EncryptedMedia` ne la rend que dans un `MediaRef` (`into_ref`), qui ne voyage que dans le message MLS (02-05) avec l'empreinte SHA-256 du blob et la taille.
- **Flux** : `MediaEncryptor` et `MediaDecryptor` traitent le fichier par morceaux, sans le garder en entier. Le déchiffrement rend chaque bloc une fois son étiquette vérifiée, et retient toujours le dernier bloc jusqu'à `finish`. C'est `finish` qui vérifie le marqueur de fin, l'empreinte et la taille. En cas d'erreur, l'app jette tout ce qui est déjà sorti.

**Reste pour plus tard**

- L'exposition au web (02-10) et l'envoi du blob (06-01).
