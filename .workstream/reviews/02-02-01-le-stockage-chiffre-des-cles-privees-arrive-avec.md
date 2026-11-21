---
type: Review
title: "Le stockage chiffré des clés privées arrive avec la 02-06"
description: "Le stockage chiffré des clés privées arrive avec la 02-06"
tags: [review]
status: draft
state: done
parent: stories/02-02-identite-de-compte-et-d-appareil.md
review: suggestion
created: 2026-11-20
commits: [e5901944afafe80c2c26f360d6ed3042298ce799]
---

# Le stockage chiffré des clés privées arrive avec la 02-06

Le stockage chiffré des clés privées arrive avec la 02-06

Le critère « les clés privées restent dans la base locale chiffrée » n'est rempli qu'à moitié : aucune fonction publique ne rend, ne sérialise ni n'affiche une clé privée (`AccountKey`, `DeviceKey` et `Device` ne l'exposent pas, et elle est effacée de la mémoire à la libération), mais elles ne vivent qu'en mémoire. La base locale chiffrée n'existe pas encore (story 02-06). Quand elle arrivera, leur sérialisation restera `pub(crate)`, réservée au stockage.

Deux autres points à valider :
- **Le lien entre la clé de compte et `pseudo@domaine`** n'est pas prouvé par le certificat. `from_credential` rend `account_key`, et l'appelant le compare à la clé qu'il connaît : celle de l'annuaire (03-01) ou des codes de sécurité (02-09).
- **La clé de compte vit sur chaque appareil du compte.** C'est un appareil déjà lié qui certifie le nouveau (03-04), et elle part dans la sauvegarde (08).

## Correction

```workstream
commits
```

- 
