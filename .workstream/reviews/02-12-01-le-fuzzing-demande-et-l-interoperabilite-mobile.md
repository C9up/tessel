---
type: Review
title: "Le fuzzing demandé et l’interopérabilité mobile ne sont pas présents"
description: "Le fuzzing demandé et l’interopérabilité mobile ne sont pas présents"
tags: [review]
status: draft
state: todo
parent: stories/02-12-tests-croises-et-fuzzing.md
review: rework
created: 2026-11-21
---

# Le fuzzing demandé et l’interopérabilité mobile ne sont pas présents

# Le fuzzing demandé et l’interopérabilité mobile ne sont pas présents

`crates/tessel-core/tests/fuzz.rs` est un bon test mutationnel déterministe, mais ce n’est pas `cargo fuzz` et aucun fuzz target libFuzzer n’est intégré à `pnpm check`, contrairement au critère explicite. Le test croisé oppose WASM au cœur natif lancé comme processus, pas à la liaison Android, encore bloquée en 02-13. La story doit rester ouverte jusqu’à réalisation ou modification explicite de ces deux critères.

## Correction

```workstream
commits
```

- 
