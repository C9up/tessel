# Tessel

Messagerie chiffrée de bout en bout, open source, sur le web et sur mobile. Tessel veut remplacer WhatsApp et reprend de Discord ses espaces, salons et rôles. Le serveur ne lit rien : il ne fait qu'acheminer des enveloppes qu'il ne sait pas ouvrir.

Pourquoi et comment : [la vision](.workstream/vision.md). Le travail est suivi dans [`.workstream/`](.workstream/).

## Organisation

| Dossier | Contenu |
|---|---|
| `apps/server` | App [Ream](https://github.com/C9up/ream) : l'API (`comet`), le temps réel (`relay`) et la coquille web (Aurora / Nebula) |
| `apps/mobile` | App Android (puis iOS) avec Expo |
| `crates/tessel-core` | Le cœur Rust : MLS, base locale chiffrée, file d'envoi, chiffrement des médias |
| `packages/protocol` | Le contrat client-serveur, partagé par le serveur et les clients |

## Lancer

Prérequis : Node ≥ 24, pnpm 12, Rust ≥ 1.90 avec la cible `wasm32-unknown-unknown`, Podman, `podman-compose`, et les binaires de [BlackGate](https://github.com/C9up/blackgate) (build release ; chemin par défaut dans `scripts/stack.sh`, ou `BLACKGATE_BIN` / `BLACKGATE_CLI`).

PostgreSQL n'est joignable qu'à travers BlackGate, en TLS mutuel, en développement comme en production. Deux rôles : `tessel_owner` possède le schéma et joue les migrations, `app` fait tourner l'application et ne possède rien.

```bash
pnpm install
cp apps/server/.env.example apps/server/.env
pnpm --filter @tessel/server exec ream generate:key
pnpm infra      # PostgreSQL + BlackGate (scripts/stack.sh) et KeyDB ; écrit les lignes DB_* du .env
pnpm --filter @tessel/server migrate   # sous tessel_owner (scripts/migrate.sh)
pnpm dev        # serveur + coquille web
pnpm check      # lint, types, tests TS et Rust, audit des dépendances
pnpm infra:down # arrête tout et efface les certificats et mots de passe de la pile
```

Les colonnes que BlackGate chiffre sont déclarées dans `deploy/blackgate/columns.yaml` avant la migration qui les crée, puis `pnpm infra:reload`. Les colonnes laissées en clair sont listées, avec leur raison, dans `deploy/blackgate/clear-columns.yaml`.

Ports de développement, choisis libres par rapport aux autres apps Ream de la machine :

| Service | Port |
|---|---|
| Serveur (API + web) | 3460 |
| BlackGate (la base, pour l'application) | 5460 |
| PostgreSQL en direct (tests seulement) | 5461 |
| KeyDB | 6460 |

## Licence

À choisir (story 01-07).
