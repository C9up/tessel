---
type: Journal
title: "Story 04-02 : ordre des commits par groupe"
status: draft
created: 2026-11-21
---

# Story 04-02 : ordre des commits par groupe

Commit d1840e3. `pnpm check` vert : Rust 110 + 8 + 2, web 34, serveur 55 (4 nouveaux, stables sur 3 passages), protocole 13. Avant cela, commit fdcdd3d : passage du serveur aux versions auditées de @c9up (ream 0.2.31, atlas 0.3.24, blackhole 0.1.28…), sans aucun changement de code.

## Ce qui est fait
- **Un commit gagne par epoch** : chaque groupe a sa ligne dans `delivery_groups` (epoch attendue, prochain numéro de séquence). Une enveloppe de groupe est placée en une seule requête :
  - la ligne du groupe est verrouillée ;
  - l'epoch est comparée à celle de l'enveloppe ;
  - un commit fait passer le groupe à l'epoch suivante ;
  - l'enveloppe prend le prochain numéro, puis est rangée avec ses destinataires.
- **Refus nommé** : `E_DELIVERY_STALE_EPOCH` (-32014), pour un second commit de la même epoch et pour tout message d'une epoch que le groupe a quittée.
  - Pour un commit, le client appelle `commitRejected`, traite ce qui est arrivé, puis `retryCommit`.
  - Pour un message, il rattrape puis le renvoie : `nextOutbound` le rechiffre dans la nouvelle epoch sous le même `SendId`.
- **Numéros de séquence** : consécutifs par groupe (le *Welcome* n'en a pas, puisqu'il n'est d'aucun groupe). Rendus par `delivery.send` et portés par `delivery.fetch`. Les identifiants d'enveloppe sont tirés après le verrou, donc dans le même ordre que les numéros de séquence.
- **L'auteur ne reçoit jamais son commit** : il n'est pas dans les destinataires (04-01). Un commit renvoyé sous le même `SendId` garde sa place, sans faire avancer le groupe deux fois.
- **Test de concurrence avec de vrais groupes MLS** : Alice et Bob ajoutent chacun quelqu'un au même instant. Un commit est accepté, l'autre refusé, rejoué après rattrapage, puis accepté à l'epoch suivante. Les quatre appareils finissent à l'epoch 3 avec 4 membres.

## Écart à la story : pas d'`eclipse`
La story demandait un verrou par groupe avec `eclipse`. J'ai pris le verrou de ligne de PostgreSQL : la comparaison d'epoch, l'avancement, le numéro et le rangement sont une seule instruction, atomique sur toutes les instances du serveur.
- Un verrou `eclipse` (Redis) autour de la même opération ajouterait KeyDB sur le chemin de chaque commit, et une seconde source de vérité sur l'epoch.
- Le risque qu'un bail expire pendant l'écriture disparaît.
- **À trancher par le propriétaire** : garder ce choix, ou ajouter `eclipse` quand même, pour l'exemple dans Ream.

## Choix faits seul
- **La première enveloppe vue fixe l'epoch d'un groupe.** Le serveur ne voit pas la création du groupe, qui est locale ; il apprend son epoch de son premier commit ou message.
- **Un message (et pas seulement un commit) d'une epoch passée est refusé.** Les destinataires restent ainsi dans une seule epoch, et le cœur sait déjà rechiffrer.

## Registre des métadonnées (story 15-02)
- `delivery_groups.group_id`, `epoch` et `next_seq` : le rythme des changements d'un groupe et son nombre de messages, sans rien sur leur contenu.
- `envelopes.seq` : la place d'une enveloppe dans son groupe.
