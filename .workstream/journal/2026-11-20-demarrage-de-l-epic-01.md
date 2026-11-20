---
type: Journal
title: "Démarrage de l'epic 01"
status: draft
created: 2026-11-20
---

# Démarrage de l'epic 01

Début du code le 20.11.2026 (« on attaque le projet »).

**01-01 (review)** : dépôt git `main`, monorepo pnpm 12 + turbo, workspace Cargo (`crates/*`, édition 2024), Biome 2.6 (Markdown, YAML, `package.json`, `.workstream` et configs d'agents exclus), squelettes `crates/tessel-core` (`version()`) et `packages/protocol`, README, `AGENTS.md` du projet.
- *Incident* : un premier `biome check --write` a reformaté (indentation seulement) `.claude/settings.local.json`, `.gemini/settings.json`, `.mcp.json` et `opencode.json`. Contenu vérifié identique (JSON valide), fichiers exclus depuis.

**Écart avec l'epic 01 : pas d'`apps/web`.** Dans Ream, l'app sert elle-même ses pages Aurora (nadeo, hugodubois.ch) : la coquille web vit dans `apps/server` (`resources/`, à venir avec 12-01). Deux apps à déployer pour une coquille statique n'apporteraient rien.

**01-02 (review)** : `apps/server` part du gabarit `ream new --template web` (ream-cli 0.3.20, @c9up/ream 0.2.30), refait à la forme nadeo : `app/modules/<module>/{domain,application,infrastructure,http}`, `shared/` (`DomainEvent` horodaté par chronos, outbox + relais + garde de déduplication KeyDB repris de nadeo sans tenant), module `system` (`GET /health`), migration `outbox`. Test d'architecture repris de nadeo (table des propriétaires réduite à `shared: [outbox]`), vérifié en y glissant une violation (domaine qui importe atlas → rouge). 6 tests verts, `tsc` vert, serveur lancé et `/health` répond. Session cookie et `auth_middleware` du gabarit retirés : l'authentification se fera par la clé d'appareil (03-02).

**Lieur pnpm** : `nodeLinker: hoisted` (copié de liste_course) abandonné pour le lieur isolé par défaut. ream-cli ne cherche `@swc-node/register` et `@c9up/ream` que dans `./node_modules` de l'app → ticket `ream-cli` bug 033. Expo supporte le lieur isolé depuis le SDK 54.

**01-03 (en cours)** : `compose.yaml` (Postgres 17, KeyDB) via `podman-compose` (le socket podman est inactif, `podman compose` passe par docker-compose et en a besoin), scripts `pnpm infra` / `infra:down`, ports 3460 / 5460 / 6460 vérifiés libres et notés au README. Restent :
- **BlackGate** : Klinivia le place devant Postgres en TLS mutuel (`scripts/stack.sh`, deux rôles). Question posée au propriétaire. L'app n'en sait rien, donc seule la config de connexion changera.
- **S3** : reporté à 06-01, où `archive` est branché ; un service que rien n'utilise n'apporte que du bruit.
