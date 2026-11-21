---
type: Journal
title: "Story 05-03 : vérifier un interlocuteur"
status: draft
created: 2026-11-21
---

# Story 05-03 : vérifier un interlocuteur

Le cœur savait déjà tout (story 02-09 : code de sécurité, QR, `markVerified`, `trust`, `acknowledgeDevice`). La 05-03 branche cela dans `@tessel/messenger`, partagé par le web et le mobile, à l'échelle de la conversation.

## Ce qui est livré

- `CoreDevice` porte désormais le `certificate` (le cœur le donnait déjà ; les deux liaisons aussi).
- `safetyNumber(group, account)` : le code avec un membre du groupe, pris sur le premier appareil de ce compte. `safetyGroups(digits)` le coupe en 12 groupes de 5 pour l'écran.
- `markVerified(group, account)` après comparaison à l'écran ; `verifyScanned(group, account, qr)` vérifie en un geste : le QR doit correspondre à l'un des appareils du compte dans le groupe (celui-là est marqué), sinon refus du cœur (`safety:mismatch`…) et rien ne change.
- `verification(group)` : pour chaque autre compte de la conversation, l'état à afficher (`verified`, `unverified`, `new-device`, `key-changed`), le pire de ses appareils.
- `acknowledgeDevices(group, account)` : l'utilisateur a vu les nouveaux appareils d'un contact vérifié.
- `onAlert(listener)` : alertes en direct, `key-changed` (le code a changé : revérifier), `new-device`, `device-removed` (`verifiedRemoved` de la 03-05). Levées à la réception d'un commit, à l'entrée dans un groupe (Welcome) et à l'invitation (l'appareil qui invite ne reçoit pas son propre commit). Les écouteurs tournent en microtâche : une exception de l'interface n'interrompt pas le traitement de la boîte.

## Tests

- messenger : 6 tests de plus (17).
- Bout en bout (`delivery/__tests__/verification.test.ts`), vrais cœurs, vrai serveur : même code des deux côtés, son propre QR refusé, celui de Bob accepté ; Bob lie une tablette → alerte `new-device` quand Alice crée un groupe avec lui, acquittée ; Bob supprime son compte, le nom est repris sous une autre clé (réservation levée en base, aide `endReservation`) → alerte `key-changed` et code différent.

## Choix faits seul

- **L'écran** (affichage du code, caméra pour scanner, badge « vérifié ») revient au client web (epic 12) et à l'appli Android (epic 13), comme pour la 05-01 : ils lisent `verification()` et écoutent `onAlert`.
- **Alerter sans bloquer** : un contact vérifié dont la clé change est signalé, la conversation continue (comme Signal). L'état `key-changed` reste affiché tant qu'on n'a pas revérifié : l'alerte ne se perd pas si l'écran n'écoutait pas.
- **Un appareil glissé sous une autre clé** n'a pas de cas à part : soit le code affiché diffère (on le voit en comparant), soit, après vérification par l'appareil légitime, l'intrus reste `key-changed`.
