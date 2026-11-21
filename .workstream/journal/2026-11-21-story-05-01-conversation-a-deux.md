---
type: Journal
title: "Story 05-01 : conversation à deux"
status: draft
created: 2026-11-21
---

# Story 05-01 : conversation à deux

Commit 14a1b8e. `pnpm check` vert : Rust 111 + 8 + 2 (1 nouveau), web 34, serveur 65 (1 nouveau, de bout en bout), protocole 18, messenger 4 (nouveau paquet).

## Ce qui est fait
- **La liste des conversations est dans le cœur** (`Client::conversations`) : chaque groupe, le plus récent en tête (l'heure d'envoi du dernier message), avec le nombre de messages d'autres comptes non encore lus (dans la fenêtre récente). `mark_read` déplace le marqueur de lecture du groupe : il est scellé dans la base de l'appareil et n'est jamais envoyé. L'accusé de lecture, en 05-04, sera un message que l'utilisateur choisit d'envoyer. Exposé par le WASM et `core-web` (`conversations`, `markRead`).
- **Nouveau paquet `@tessel/messenger`**, partagé par le web et le mobile, défini sur une interface de ce qu'il attend du cœur (`MessengerCore`) :
  - `start(account)` réserve les KeyPackages des appareils du contact et des autres appareils de son propre compte. Il vérifie chacun avec le cœur et exige qu'il soit du compte demandé, pour que le serveur ne puisse pas glisser un autre appareil. Il crée ensuite le groupe et envoie les *Welcome*. Le commit d'un groupe neuf ne va à personne, puisque l'appareil est encore seul dedans ;
  - `write(group, text)` met en file dans le cœur, puis envoie dans l'ordre. Si le serveur refuse un message parce que le groupe est passé à une autre epoch, le messenger rattrape et renvoie ;
  - `receive(envelope)` remet ce qui arrive au cœur (un *Welcome* fait rejoindre le groupe) ; c'est la fonction `deliver` de l'`Inbox`.
- **Test de bout en bout sur le vrai serveur** :
  - Alice a un téléphone et une tablette liée, Bob un téléphone ;
  - Alice démarre la conversation depuis son téléphone et écrit, Bob répond ;
  - les trois appareils ont les deux messages, dans l'ordre ;
  - la liste compte un non-lu sur la tablette d'Alice et sur le téléphone de Bob, et plus aucun une fois lu.

## Ce qui n'est pas dans cette story
- **Démarrer depuis un lien** : les liens d'invitation sont l'epic 09. On démarre ici depuis un nom de compte, c'est-à-dire depuis un contact.
- **L'écran** (liste, compteur visible) : l'epic 12 (client web) l'affichera à partir de `conversations()`.
- **Envoyer la clé de profil au démarrage de la conversation** (03-06) : il suffira d'appeler `shareProfile` après `start`. Je l'ai laissé à l'interface, qui décide quand montrer son profil.

## Choix faits seul
- **Le tri de la liste suit l'heure d'envoi du dernier message**, celle de l'expéditeur : une horloge fausse peut déplacer une conversation. L'ordre du serveur n'est comparable qu'à l'intérieur d'un groupe, pas d'un groupe à l'autre.
- **Les non-lus ne se comptent que dans la fenêtre récente** du cœur (`EDITABLE_MESSAGES`) : au-delà, l'interface affiche « 99+ ».
