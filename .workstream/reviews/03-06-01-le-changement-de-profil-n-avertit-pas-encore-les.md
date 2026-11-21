---
type: Review
title: "Le changement de profil n’avertit pas encore les contacts de bout en bout"
description: "Le changement de profil n’avertit pas encore les contacts de bout en bout"
tags: [review]
status: draft
state: todo
parent: stories/03-06-profil-chiffre.md
review: rework
created: 2026-11-21
---

# Le changement de profil n’avertit pas encore les contacts de bout en bout

# Le changement de profil n’avertit pas encore les contacts de bout en bout

Le cœur sait chiffrer le profil, partager la clé par MLS et produire `Effect::ProfileUpdated`; le serveur ne garde que le blob opaque. Mais le journal confie encore à un hôte inexistant la boucle qui appelle `share_profile` dans chaque groupe et transporte ces messages. Sans cette orchestration, changer de profil ne prévient pas réellement les contacts : le troisième critère reste incomplet.

## Correction

```workstream
commits
```

- 
