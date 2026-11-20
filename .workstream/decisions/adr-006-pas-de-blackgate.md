---
type: Décision
title: "ADR-006 — Pas de BlackGate : la base ne garde que du chiffré ou de l'opaque"
description: "BlackGate est retiré : produit propriétaire, incompatible avec un projet MIT que chacun doit pouvoir lancer. PostgreSQL reste en TLS mutuel ; la protection des données vient du chiffrement de bout en bout et de la minimisation, comme chez Signal."
tags: [adr, serveur, base-de-donnees]
status: draft
decision_status: "Acceptée"
created: 2026-11-20
---

# ADR-006 — Pas de BlackGate : la base ne garde que du chiffré ou de l'opaque

## Contexte

Depuis la story 01-03, PostgreSQL n'était joignable qu'à travers BlackGate, un proxy qui chiffre des colonnes choisies (montage repris de Klinivia ; choix « BlackGate dès le dev » du 20.11.2026). En préparant la CI (story 01-06), trois problèmes sont apparus :

- BlackGate est **propriétaire** : dépôt privé, aucune licence (ADR-005). Quelqu'un qui clone Tessel (MIT, public) ne peut ni lancer les tests ni installer sa propre instance (story 14-04), et la CI publique ne peut pas l'obtenir.
- Il **ne protège pas contre l'hébergeur**, la menace n°1 de la vision : l'opérateur détient sa clé maîtresse.
- Le contenu des conversations est déjà chiffré de bout en bout : BlackGate ne protégeait que des métadonnées, et seules les trois colonnes de l'outbox étaient chiffrées.

Les autres messageries ne chiffrent pas leurs colonnes par un proxy. Signal garde le moins possible :
- contenu, profils et groupes chiffrés par les clients ;
- expéditeur scellé ;
- réponses aux réquisitions limitées à la date de création du compte et à la dernière connexion.

WhatsApp chiffre le contenu, mais garde les métadonnées en clair chez Meta. Pour Matrix, les métadonnées sur le serveur d'accueil sont son point faible connu.

## Décision (propriétaire, 20.11.2026 : « non on le vire, c'est un produit propriétaire »)

- **BlackGate est retiré de Tessel** : code, scripts, configuration, CI, déploiement.
- **Tout ce qui est sensible est chiffré par les appareils, point.** La base ne reçoit que :
  - du chiffré de bout en bout (enveloppes MLS, médias, profils, sauvegardes) ;
  - des identifiants opaques (aléatoires, sans rien dire de leur contenu) ;
  - ce que le serveur doit comparer ou trier (horloges, compteurs, états de file).
  Un événement de l'outbox ne nomme que des identifiants opaques, jamais un contenu.
- **PostgreSQL reste en TLS mutuel** (`sslmode=verify-full` et certificat client), en dev, en CI et en prod : une connexion sans TLS ou sans certificat est refusée, et un test le prouve. `scripts/stack.sh` produit les certificats avec `openssl`.
- **Au repos**, la base repose sur le chiffrement du disque de l'hôte (story 14-01).
- **Ce que le serveur sait est au registre des métadonnées** (story 15-02), chaque colonne avec sa raison. Le levier contre l'hébergeur est de stocker moins : registre (15-02), expéditeur scellé (18-02).

## Conséquences

- `deploy/blackgate/` est supprimé. Sa liste des colonnes laissées en clair, avec leur raison, est reprise ici pour le registre (15-02) :
  - **clé** (la base génère ou compare l'identifiant) : `outbox.id` ;
  - **file** (la base trie, compare ou calcule ; cela dit quand et combien, rien de ce qui est dit) : `outbox.created_at`, `outbox.available_at`, `outbox.attempts` ;
  - **anciennement chiffrées par BlackGate**, désormais en clair, sous la règle des identifiants opaques : `outbox.event_type`, `outbox.payload`, `outbox.last_error`.
- Plus de port « PostgreSQL direct » : la base écoute sur 5460, en TLS mutuel.
- La CI (01-06) lance la même pile (`scripts/stack.sh`, podman) que le développement.
- ADR-004 : la règle « une colonne nouvelle est chiffrée par BlackGate » est remplacée par la règle des identifiants opaques et le registre.
- ADR-005 : BlackGate sort de la table des dépendances.

## Sources

- Signal, réponses publiées aux réquisitions (signal.org/bigbrother) ; documentation de l'expéditeur scellé et des groupes v2.
- WhatsApp Encryption Overview (livre blanc), pour ce que WhatsApp garde en clair.
