---
type: Journal
title: "Story 02-09 : codes de sécurité"
status: draft
created: 2026-11-20
---

# Story 02-09 : codes de sécurité

**Choix**

- **Dérivation de Signal** (empreintes numériques) : pour chaque personne, SHA-512 itéré 5200 fois sur la clé de compte et le compte, avec l'étiquette « Tessel 1.0 safety number ». Les 30 premiers octets donnent 30 chiffres, 5 par 5. Le code fait 60 chiffres : les deux moitiés, la plus petite d'abord, donc le même code des deux côtés.
- **Identités de compte, pas d'appareil** : ajouter un appareil ne change pas le code ; le code change si la clé de compte change.
- **QR** : `[version 1][mon empreinte, 32 octets][la tienne, 32 octets]`. Celui qui scanne vérifie que c'est l'inverse des siennes (`check_scanned`). Un code d'une autre conversation, ou son propre QR, est refusé (`Mismatch`).
- **Contacts vérifiés**, scellés dans la partition d'état (enregistrements `verified`). Pour chaque contact, le cœur garde la clé de compte et les appareils connus au moment de la vérification : l'appareil montré, plus ceux du même compte, sous la même clé, déjà présents dans les groupes.
  - `trust(certificat)` rend `Unverified`, `Verified`, `NewDevice` (appareil ajouté depuis, à signaler) ou `KeyChanged` (autre clé de compte : la vérification ne tient plus, il faut revérifier).
  - `acknowledge_device` fait taire le signalement d'un nouvel appareil ; il est refusé si la clé a changé.
  - `unverify` retire la vérification.
- L'app appelle `trust` sur les appareils ajoutés par un commit, sur les membres d'un groupe rejoint et sur l'expéditeur d'un message. Plutôt que de dupliquer des alertes dans `Incoming`, un seul point de vérité.

**Décisions de 02-06 et 02-07 tranchées** (« fais au mieux ») :

- Le critère Keystore Android passe à la 02-13 (critère ajouté là-bas).
- La fenêtre de 1024 messages modifiables est gardée.
- Un message à la fois par groupe, en attendant que l'API du serveur (04-03) permette l'envoi par lots.
- L'ADR-007 reste « Proposée », en attendant la validation du propriétaire.
