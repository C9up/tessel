---
type: Review
title: "Choix du format des messages à valider (commit 49509ab)"
description: "Choix du format des messages à valider (commit 49509ab)"
tags: [review]
status: draft
state: review
parent: stories/02-05-messages-applicatifs.md
review: suggestion
created: 2026-11-20
commits: [e5901944afafe80c2c26f360d6ed3042298ce799]
---

# Choix du format des messages à valider (commit 49509ab)

Choix du format des messages à valider (commit 49509ab)

- **Format écrit à la main**, pas avec `tls_codec` : le `VLBytes` de `tls_codec` panique en debug sur une entrée tronquée, et un lecteur borné de 60 lignes se teste entièrement. Il est testé contre toutes les troncatures, les octets modifiés et 20 000 entrées au hasard.
- **Réponse = champ `reply_to`** d'un texte ou d'un média, pas un genre à part : on peut répondre par une photo.
- **Remplissage** : 256 octets au minimum, puis le palier Padmé (surcoût de 6 % au plus au-delà de 256). Tous les messages courts ont la même taille chiffrée. Le remplissage doit être exact et nul : pas de canal caché.
- **Texte limité à 65 535 octets** UTF-8 : au-delà, l'appli découpe ou envoie un fichier.
- **Un message illisible n'est pas tu** : `Incoming::Message` rend l'expéditeur authentifié avec `Err(...)`. L'appli peut afficher « message de X, illisible » ou « mettez Tessel à jour » pour une version plus récente.
- **Édition et suppression** : seul l'auteur du message visé peut le changer. Le cœur ne garde pas encore l'historique, donc la vérification (comparer l'expéditeur des deux messages) arrive avec la base locale, story 02-06.
- **Heure d'envoi** : celle de l'expéditeur, qui peut mentir. L'ordre du serveur fait foi pour l'affichage.
- **La clé d'un média** voyage dans le message et s'efface à la libération. Aucune fonction publique ne la rend ; la story 02-08 la créera et l'utilisera.

## Correction

```workstream
commits
```

- 
