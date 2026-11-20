---
type: Journal
title: "Story 02-07 : file d'envoi hors ligne"
status: draft
created: 2026-11-20
---

# Story 02-07 : file d'envoi hors ligne

**Choix**

- **Tout message passe par la file**, en ligne ou non : `queue_message` l'écrit dans l'historique et en fin de file (enregistrements `outbox` de la partition d'état, scellés). L'hôte envoie avec `next_outbound`, puis `outbound_sent` à l'accusé du serveur. Un seul chemin : « hors ligne » n'est qu'une file qui attend.
- **Chiffré au moment de l'envoi, pas à la mise en file.** Un message peut attendre longtemps : entre-temps des membres arrivent ou partent, et MLS interdit d'envoyer tant que des propositions attendent leur commit (`GroupError::PendingProposals`). Chiffrer au dernier moment, dans l'epoch courante, écarte un membre parti et n'envoie jamais dans une epoch que les membres ne lisent plus. La file garde donc le texte clair, scellé comme l'historique.
- **Une seule fois** : identifiant d'envoi `SendId` aléatoire (distinct de `MessageId`, que seuls les membres voient), que le serveur utilise pour ignorer un doublon (critère ajouté à la 04-01). Un renvoi après accusé perdu est rechiffré (nouvelle génération) mais rangé une fois. Filet de sécurité : les destinataires ignorent déjà un `MessageId` réutilisé (`DuplicateId`).
- **Ordre par groupe** : seule la tête est rendue ; la suivante après l'accusé. Les groupes sont indépendants. Pas d'envoi en rafale pour l'instant (un aller-retour par message) ; à revoir avec l'API du serveur (04-03) si le vidage d'une longue file est lent.
- **Appareil retiré du groupe** : sa file est effacée et `Incoming::Commit.unsent` rend les messages qui ne partiront jamais, pour que l'app les marque.
- **Web** : `OutboxSender` (`@tessel/core-web`) vide la file au démarrage, à la demande, sur l'événement `online`, et après un échec avec des délais croissants (1 s → 60 s). Un groupe bloqué ne retient pas les autres. Il attend la liaison du client (02-10) et le transport (04-03) derrière deux interfaces, `Outbox` et `Relay`.

**Limites**

- La déduplication côté serveur n'existe pas encore (04-01) ; d'ici là, le filet `DuplicateId` des destinataires évite le double affichage.
- L'hôte doit traiter les commits entrants avant de vider la file, sinon le message part dans une epoch dépassée. Le serveur (04-02) pourra le refuser ; il sera alors rechiffré au renvoi.
