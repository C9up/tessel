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

Prérequis : Node ≥ 24, pnpm 12, Rust ≥ 1.90 avec la cible `wasm32-unknown-unknown`, Podman ou Docker.

```bash
pnpm install
cp apps/server/.env.example apps/server/.env   # puis : DB_PASSWORD, et `pnpm --filter @tessel/server exec ream generate:key`
pnpm infra      # Postgres et KeyDB (podman-compose)
pnpm --filter @tessel/server migrate
pnpm dev        # serveur + coquille web
pnpm check      # lint, types, tests TS et Rust, audit des dépendances
pnpm infra:down
```

Ports de développement, choisis libres par rapport aux autres apps Ream de la machine :

| Service | Port |
|---|---|
| Serveur (API + web) | 3460 |
| Postgres | 5460 |
| KeyDB | 6460 |

## Licence

À choisir (story 01-07).
