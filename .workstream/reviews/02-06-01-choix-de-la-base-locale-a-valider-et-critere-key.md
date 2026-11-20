---
type: Review
title: "Choix de la base locale à valider, et critère Keystore renvoyé à la 02-13 (commi…"
description: "Choix de la base locale à valider, et critère Keystore renvoyé à la 02-13 (commi…"
tags: [review]
status: draft
state: todo
parent: stories/02-06-base-locale-chiffree.md
review: suggestion
created: 2026-11-20
---

# Choix de la base locale à valider, et critère Keystore renvoyé à la 02-13 (commi…

Choix de la base locale à valider, et critère Keystore renvoyé à la 02-13 (commit e590194)

La conception est décrite dans l'ADR-007, avec le statut « Proposée ».

**Ce qui est fait**
- **Contrat unique dans le cœur** : `Changes`, `Record` et le trait `Store`. Le cœur scelle chaque enregistrement ; l'hôte range des octets opaques et applique les changements en une transaction, avant d'agir sur la sortie de l'appel.
- **Deux implémentations** :
  - `SqliteStore` en natif, avec la fonctionnalité `sqlite` et `secure_delete` ;
  - `IndexedDbStore` dans `@tessel/core-web`.
  - Chacune a des migrations versionnées et testées : ordre, échec qui ne change rien, base plus récente refusée.
- **Web** : la clé de la base est tirée par le cœur (`generateStoreKey`), puis scellée par une clé WebCrypto non extractible gardée dans IndexedDB. Un test vérifie que `exportKey` est refusé.
- **Auteur d'une édition ou d'une suppression** : la règle est vérifiée sur les 1024 derniers messages de chaque groupe. Les tests couvrent Carol qui modifie le message de Bob (ignoré), le second appareil de Bob (appliqué), un identifiant réutilisé (le message de Bob n'est pas écrasé) et la suppression (le texte quitte la base, une pierre tombale reste).
- **Redémarrage** : le client reprend ses groupes, sa réserve de *KeyPackages* et une intention de commit en attente. Un message déjà lu ne se déchiffre pas deux fois.

**À valider**
1. **Critère « Android : clé protégée par le Keystore »** : il ne peut se faire que dans la liaison mobile, qui n'existe pas encore. Je propose de le déplacer dans la 02-13 ; `SqliteStore::host_value` prévoit déjà l'emplacement de la clé scellée.
2. **Fenêtre d'édition** : 1024 derniers messages par groupe, plutôt qu'une durée. Au-delà, une édition ou une suppression est ignorée.
3. **Pas de SQLCipher** : le même scellement dans le cœur sert aux deux hôtes.
4. **Limites** (ADR-007) : sur le web, la clé n'est pas protégée contre le vol du profil du navigateur ; rien ne détecte le retour arrière d'une ancienne copie de la base.

## Correction

```workstream
commits
```

- 
