---
type: Journal
title: "Story 02-12 : tests croisés, vecteurs MLS et fuzzing"
status: draft
created: 2026-11-21
---

# Story 02-12 : tests croisés, vecteurs MLS et fuzzing

Commit `1e458f5`. `pnpm check` vert en 25 s : 87 + 8 + 2 tests Rust, 28 tests web, 11 serveur, 2 protocole.

## Commencée avant la 02-13
La 02-13 est bloquée (journal du 21.11). Tout ce qui ne demande pas le téléphone est fait ici. Le test croisé tourne avec le cœur compilé en natif sur le PC. Le même test sur Android viendra avec la 02-13.

## Test croisé WASM ↔ natif
- `examples/interop_peer.rs` : des appareils du cœur compilé en natif, pilotés ligne par ligne.
- `packages/core-web/src/interop.test.ts` : deux appareils web (WASM) et deux natifs dans un même groupe.
  - Un appareil web crée le groupe et ajoute les deux natifs.
  - Les messages passent dans les deux sens, avec des caractères non ASCII.
  - Un appareil natif ajoute un appareil web : tout le monde passe à la même époque.
  - Le dernier arrivé est lu par les quatre.
- La différence de taille de mot (32 bits en WASM, 64 en natif) est donc couverte.

## Vecteurs de test MLS officiels
- `tests/mls_vectors.rs` vérifie, par l'API publique, les vecteurs du groupe de travail MLS (tels qu'OpenMLS v0.9.2 les livre, copiés dans `tests/vectors/` avec leur provenance) :
  - `crypto-basics`, suite 1 : RefHash, ExpandWithLabel, DeriveSecret, DeriveTreeSecret, SignWithLabel, EncryptWithLabel ;
  - `messages` : 25 vecteurs sur 300, décodés puis réencodés à l'octet près.
- **Non couvert ici** : les vecteurs qui demandent l'intérieur d'OpenMLS (calcul de l'arbre, *key schedule*, *secret tree*, clients passifs). Ils tournent dans la suite de tests d'OpenMLS, sur la version épinglée. Lancer cette suite sur la machine a été refusé par le contrôle des permissions : il aurait fallu compiler et exécuter le code fraîchement téléchargé. C'est au propriétaire de décider.

## Fuzzing
- `tests/fuzz.rs` couvre chaque entrée venue de l'extérieur : message MLS, *Welcome*, *KeyPackage*, certificat, message applicatif, blob de média, secret de média scellé, QR de sécurité.
- Pour chacune : des milliers de variantes abîmées d'une entrée valide (bit inversé, octet changé, longueurs extrêmes, troncature, suppression, insertion, duplication), plus du bruit pur.
- Deux propriétés sont vérifiées :
  - le cœur ne panique jamais ;
  - il n'accepte jamais une entrée abîmée là où une signature ou une étiquette d'authentification la protège.
- Deux tests vérifient que le détecteur signale bien une panique et une acceptation.
- 400 essais par cible dans `pnpm check`, de façon déterministe. `TESSEL_FUZZ_RUNS` et `TESSEL_FUZZ_SEED` lancent plus long.
- Passe longue faite : 3 graines × 20 000 essais par cible en release, puis 5 000 en debug (le mode où tls_codec panique sur une entrée tronquée). **Aucun défaut trouvé.**
- **Écart avec le critère** : ce n'est pas `cargo fuzz`. Il faut la toolchain nightly et cargo-fuzz, absents de la machine et de la CI. Les installer revient au propriétaire. Avec cargo-fuzz, chaque cible deviendrait un `fuzz_target!` d'une ligne sur la même fonction ; libFuzzer apporterait la couverture de code, que ce harnais n'a pas.
