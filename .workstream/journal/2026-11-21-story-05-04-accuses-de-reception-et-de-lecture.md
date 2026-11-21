---
type: Journal
title: "Story 05-04 : accusés de réception et de lecture"
status: draft
created: 2026-11-21
---

# Story 05-04 : accusés de réception et de lecture

## Ce qui est livré

**Cœur (`crates/tessel-core/src/receipts.rs`, nouveau)**
- Un accusé est un message MLS comme un autre (le format `Receipt` existait depuis la 02-05) : le serveur voit une enveloppe de plus, rien d'autre.
- **Ce que l'appareil doit** est gardé dans l'état scellé (`owed-receipts`, par groupe) : une remise pour chaque texte/média d'un autre compte reçu, une lecture pour ce que `mark_read` vient de lire. `queue_receipts(sent_at)` les met dans la file d'envoi, par paquets de `MAX_RECEIPT` (256) : un accusé pour beaucoup de messages. Plafond de 1 024 en attente par sorte (au-delà, aucun accusé n'atteint l'index récent).
- **Les accusés reçus** sur les messages de ce compte sont rangés dans la partition d'historique du groupe (`receipt`, nommé message + compte + sorte : idempotent, une lecture n'efface pas la remise). L'historique gagne `receipts` (qui, jusqu'où) et `status` : `pending` (en file), `sent`, `delivered` (tous les autres comptes encore dans le groupe l'ont sur un appareil), `read` (tous l'ont lu).
- **Un accusé de lecture d'un autre appareil du même compte** avance le marqueur de lecture : lu sur la tablette = plus de non-lu sur le téléphone.
- **Réglage réciproque** (`set_read_receipts`, persistant, actif par défaut) : désactivé, l'appareil n'envoie plus de lecture (et oublie celles qu'il devait) et n'enregistre plus celles des autres. La remise part toujours.
- Nouvel effet `receipt` sur un message traité : l'interface rafraîchit les coches.

**Messenger** : `settle()` envoie ce qui est dû une fois un lot de la boîte traité, sans rattrapage (depuis la boîte, sinon interblocage ; un refus d'époque laisse l'accusé en file, il part au prochain envoi) ; `markRead` envoie la lecture (avec rattrapage) ; `readReceipts` / `setReadReceipts`.
**Protocole** : `Inbox` prend un 4ᵉ argument `settle`, appelé après chaque lot traité et acquitté.
**Liaisons** : `queueReceipts`, `readReceipts`, `setReadReceipts` sur le WASM, le mobile, `TesselCore` ; scénario de contrat (web 38 tests, mobile 14).

## Défaut trouvé en route (corrigé)

Avec les accusés, il y a presque toujours des messages en vol quand quelqu'un commite. Le bout en bout des groupes a cassé : Alice retire Carol, puis lit un accusé que Carol avait envoyé juste avant. Le cœur cherchait l'expéditeur dans l'arbre **actuel** (`certificate_at`) : Carol n'y était plus → `malformed`. Pire, si la place de Carol avait été reprise (Dave ajouté), le message aurait été **attribué à Dave**. Correction : l'expéditeur est lu depuis le credential qu'OpenMLS résout dans l'époque du message (`ProcessedMessage::credential`, feuilles des époques passées, `MAX_PAST_EPOCHS = 3`) ; `DeviceCertificate::from_sender_credential` revérifie la signature du compte (la liaison clé de feuille ↔ certificat est vérifiée à l'entrée de la feuille). Test de non-régression : `a_message_written_before_a_commit_stays_its_author_s_even_once_their_leaf_is_gone`.

## Budget WASM relevé (à dessein)

`wasm-size.json` : brut 1 950 000 → 2 000 000 octets (module à 1 952 908). Le gzip (637 134) reste sous son plafond de 640 000, inchangé : c'est ce que la page télécharge. Le prochain ajout notable le touchera : à surveiller.

## Tests

5 tests Rust (cycle complet, groupe à trois, réglage réciproque, synchro de lecture entre appareils, paquets et redémarrage), 1 de non-régression, 2 messenger (19), 1 protocole (19), bout en bout `delivery/__tests__/receipts.test.ts` (une remise pour 3 messages, puis la lecture, puis plus de lecture une fois désactivé). Suite complète verte (68 serveur, 124 cœur).

## Choix faits seul

- **Chaque appareil envoie sa remise** (pas de coordination entre les appareils d'un compte) ; le statut est par compte, donc idempotent. Coût : un accusé par appareil et par lot, pas par message.
- **Groupes** : « remis »/« lu » quand *tous* les autres comptes encore membres l'ont, comme WhatsApp ; le détail par compte est dans `receipts` pour un écran « infos du message ».
- **Réglage par appareil**, pas par compte (pas de synchro des réglages entre appareils pour l'instant).
- **Écran** (coches, interrupteur dans les réglages) : epic 12 / 13.

## Pour le registre des métadonnées (15-02)

Aucune colonne ni champ nouveau. Le trafic, lui, en dit un peu : une enveloppe d'un appareil peu après une réception ou une lecture laisse deviner qu'il est en ligne ou que l'utilisateur lit. Désactiver les accusés de lecture supprime le second signal, pas le premier.
