---
type: Review
title: "Choix de protocole des groupes, à valider"
description: "Choix de protocole des groupes, à valider"
tags: [review]
status: draft
state: done
parent: stories/02-04-groupes-mls.md
review: suggestion
created: 2026-11-20
---

# Choix de protocole des groupes, à valider

Choix de protocole des groupes, à valider

- **Tout part chiffré** (`PURE_CIPHERTEXT_WIRE_FORMAT_POLICY`), commits et propositions compris. Le serveur ne voit que l'identifiant aléatoire du groupe, l'epoch et le type de message. Conséquence : l'auteur d'un commit ne peut pas relire son écho, donc le serveur lui répond « accepté » ou « refusé ». Critère ajouté à 04-02.
- **Rejeu** : l'intention (ajout, retrait, commit des propositions) est gardée jusqu'à l'accusé du serveur. `retry_commit` la rejoue sur l'état à jour et ignore ce que les autres ont déjà fait (appareils déjà là ou déjà partis). Recevoir le commit gagnant annule de lui-même notre commit en attente.
- **Quitter** se fait par une proposition, qu'un autre membre fait entrer avec `commit_pending_proposals`. Qui le fait, et quand, se décidera côté application (epic 05).
- **Clés gardées 3 epochs** (`max_past_epochs`), pour lire un message chiffré juste avant un commit. Au-delà, elles sont effacées.
- **Retrait par compte ou par appareil** (`Removal`). Un appareil ne se retire jamais lui-même par commit.
- **Un commit qui fait entrer une feuille sans certificat valide est refusé.** Mais MLS ne permet pas de sauter un commit : un membre malveillant bloque alors le groupe pour tous. La conduite à tenir (signaler, recréer le groupe) reste à décider, avec la modération (epic 07).
- L'état des groupes vit en mémoire et dans le stockage mémoire d'OpenMLS jusqu'à la 02-06.

## Correction

```workstream
commits
```

- 
