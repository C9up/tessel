---
type: Journal
title: "Story 04-01 : boîtes aux lettres des appareils"
status: draft
created: 2026-11-21
---

# Story 04-01 : boîtes aux lettres des appareils

Commit 2dde26f. `pnpm check` vert : Rust 110 + 8 + 2 (3 nouveaux), web 34, serveur 51 (4 nouveaux), protocole 12.

## Ce qui est fait
- **En-tête public seulement** : `inspect_envelope`, dans le cœur, lit le type (application, proposition, commit, *Welcome*), le groupe et l'epoch d'un `MLSMessage`, rien d'autre. Un KeyPackage ou des octets quelconques sont refusés. Le serveur passe par le WASM (`readEnvelope`) : il n'analyse aucun MLS lui-même.
- **Destinataires donnés par l'expéditeur** : le serveur ne connaît pas les membres d'un groupe, ils sont dans l'état MLS chiffré. `Client::recipients(group)` donne tous les appareils membres sauf soi, y compris ses propres autres appareils. Pour un commit, la liste se prend avant l'acceptation, pour que les appareils retirés l'apprennent aussi. Un *Welcome* va aux appareils ajoutés.
- **Rangé une fois** : `envelopes` (en-tête, corps opaque, clé de déduplication) plus `deliveries` (appareil, enveloppe), dans une seule requête. Le même couple (appareil expéditeur, `SendId`) n'est rangé qu'une fois, et le même identifiant revient : le test rejoue un envoi ré-chiffré par le cœur sous le même `SendId`.
- **Chacun ne lit que le sien** : `delivery.fetch {after?}` rend les enveloppes de l'appareil appelant, dans l'ordre d'arrivée, après son curseur, au plus 100 à la fois. L'expéditeur ne reçoit jamais sa propre enveloppe.
- **Test de bout en bout avec de vrais groupes MLS** : Alice crée un groupe, ajoute Bob et envoie le *Welcome* par la boîte. Bob rejoint et lit « hello, Bob ». Carol, hors du groupe, ne reçoit rien. En base, le corps ne contient pas le texte.

## Choix faits seul
- **Pas de clé étrangère vers `devices`** : c'est la table du module identity, et la règle des modules interdit de la lire. Une enveloppe adressée à un appareil inexistant n'est jamais lue et part avec la purge (04-04).
- **Le serveur ne rend pas l'expéditeur au destinataire**, puisque MLS l'authentifie dans le message chiffré. Il ne garde l'appareil expéditeur que pour la déduplication, jusqu'à la purge.
- **Limites grossières** (1 Mio par enveloppe, 1000 destinataires, 300 envois par minute) en attendant la 04-06.

## Registre des métadonnées (story 15-02)
- `envelopes.sender_device` et `send_id` : la clé de déduplication, gardée jusqu'à la purge.
- `envelopes.kind`, `group_id`, `epoch` : l'en-tête public MLS.
- `envelopes.body` : chiffré par les appareils.
- `envelopes.created_on` : le jour.
- `deliveries` : (appareil, enveloppe).
- Ce que le serveur apprend : qui écrit dans quel groupe, et à quels appareils, sous des identifiants opaques, jusqu'à la purge. L'expéditeur scellé (18-02) réduira la part « qui écrit ».
- Compteurs `delivery_send_<appareil>` et `delivery_fetch_<appareil>` : une minute.
