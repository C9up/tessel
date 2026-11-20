# Tessel — consignes pour les agents

Tessel est une messagerie chiffrée de bout en bout (MLS). Lire [la vision](.workstream/vision.md) avant tout travail : ses principes priment sur la commodité.

## Travail

- Le travail est suivi par Workstream (`.workstream/`) : lire la story et son epic avant de coder, `doing` au début, `review` à la fin ; `done` appartient au propriétaire.
- Décisions et études : `append_journal`, ou un document dans `.workstream/decisions/`.

## Règles qui ne se discutent pas

- **Le serveur ne lit rien.** Aucun contenu, nom de groupe, nom de fichier ou profil en clair côté serveur. Ce qui exigerait de lire se fait sur l'appareil.
- **Pas de cryptographie maison.** MLS via OpenMLS, primitives standard. Toute manipulation de clé vit dans `crates/tessel-core`, jamais en TypeScript.
- **Ce que le serveur sait est au registre des métadonnées** (story 15-02) : une colonne ou un champ de journal nouveau s'y inscrit.

## Serveur : Ream, à la Ream

- Modèle `ream/apps/nadeo` : `app/modules/<module>/{domain,application,infrastructure,http}`, `start/<module>.ts` compose. Lire `nadeo/AGENTS.md` avant d'ajouter un module.
- Utiliser les paquets `@c9up` avant d'écrire une brique (`ream-dev/packages/*/README.md`). Un défaut d'un paquet se signale dans le `.workstream` de ce paquet.
- BlackGate est une boîte noire : l'app ne connaît que l'URL de sa base.
- Dates et durées : `@c9up/chronos`, jamais `Date.now()` à la main.

## Langue

Documents de travail (`.workstream/`) en français. Tout ce qui vit dans le code est en anglais : identifiants, commentaires, noms de tests, fixtures, messages d'erreur et de log, sorties des scripts, README, messages de commit (skill `code-in-english`, hook `pre-commit`).

Aucune signature d'agent dans les commits ni dans les PR : pas de `Co-Authored-By` Claude ou autre, pas de « Generated with » (skill `no-ai-signatures`, hook `commit-msg`).
