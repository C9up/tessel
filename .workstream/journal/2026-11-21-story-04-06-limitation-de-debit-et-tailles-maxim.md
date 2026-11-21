---
type: Journal
title: "Story 04-06 : limitation de débit et tailles maximales"
status: draft
created: 2026-11-21
---

# Story 04-06 : limitation de débit et tailles maximales

Commit 87668b4. `pnpm check` vert : Rust 110 + 8 + 2, web 34, serveur 64 (3 nouveaux), protocole 18.

## Ce qui est fait
- **Compteurs partagés** : le limiteur de `blackhole` compte dans KeyDB (connexion quasar `main`). Une limite tient quel que soit le nombre d'instances (04-05). Les limites déjà posées en 03-xx en profitent aussi.
- **Débit par appareil et par compte** pour `delivery.send` : 300 par minute par appareil, 600 par minute par compte, tous appareils confondus. Lier des appareils ne permet pas de contourner la limite. Test avec trois appareils d'un même compte, chacun sous sa propre limite : le 601e envoi du compte est refusé.
- **Une taille par type** (`ENVELOPE_LIMITS`) :

  | Type | Taille maximale | Raison |
  |---|---|---|
  | Message | 128 Kio | un texte de 64 Kio au plus (`MAX_TEXT`), avec son remplissage |
  | Proposition | 64 Kio | |
  | Commit | 256 Kio | |
  | *Welcome* | 512 Kio | il porte l'arbre du groupe |

  Au-delà : refus nommé `E_DELIVERY_TOO_LARGE` (-32015), avec les limites dans `data`. Rien de plus grand que le type le plus grand n'est même décodé. Le tout reste sous la limite du corps de requête (1 Mo de JSON).

## Défaut trouvé dans blackhole (ticket 043, écrit à la main dans `ream-dev/packages/blackhole/.workstream/bugs/`)
`limiter.clear()` ne vide que les limiteurs déjà construits par le processus. Avec Redis, les compteurs d'une exécution précédente ou d'une autre instance restaient en place, et le premier test de plusieurs fichiers échouait à tort. Contournement dans Tessel : `clearLimits()` (`app/modules/shared/__tests__/limits.ts`) passe par le `clear()` du store, qui efface tout le préfixe.

## Choix faits seul
- **Les chiffres** (300/600 par minute ; les tailles ci-dessus) : de quoi taper, rattraper une file d'attente, ajouter beaucoup d'appareils d'un coup ; pas de quoi noyer le serveur. Ils sont dans le code, pas encore dans la configuration.
- **Les récupérations et accusés** restent limités par appareil seulement (600 par minute) : un compte n'a pas de raison d'être freiné à la lecture.

## Coût en CI
Les tests de débit font environ 1 200 appels RPC : la suite serveur passe d'environ 60 s à 100 s.
