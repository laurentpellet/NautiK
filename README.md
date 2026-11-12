
# NautiK

Application open source de navigation maritime avancée.

## Stack technique

- Rust stable + Tokio
- Tauri 2
- Svelte 5 + TypeScript
- MapLibre GL JS
- SQLite + SQLx
- Axum + WebSocket
- Signal K, NMEA 0183, AIS
- Linux ARM64 / Raspberry Pi

## Objectifs

NautiK est une application de navigation maritime offline-first,
inspirée fonctionnellement d'OpenCPN, mais développée avec une
architecture moderne et indépendante.

Le logiciel doit fonctionner :

1. En client lourd sur Windows, Linux et macOS.
2. En mode service sur Raspberry Pi / OpenPlotter.
3. Avec une interface Web distante.
4. Sans connexion Internet pour les fonctions essentielles.

## Documents

- SPEC.md : spécifications fonctionnelles
- ARCHITECTURE.md : architecture technique
- DATA_MODEL.md : modèle SQLite
- TASKS.md : plan de développement
- AGENTS.md : instructions pour Claude Code
- SECURITY.md : sécurité et fiabilité
- TEST_PLAN.md : stratégie de tests

## Principes

- Le moteur Rust ne dépend jamais de Tauri.
- Le fonctionnement hors ligne est prioritaire.
- Les alarmes fonctionnent indépendamment de l'interface.
- Les données de navigation sont normalisées.
- Les communications externes sont sécurisées.
- Les fonctionnalités sont développées progressivement.

## Licence

À définir après audit des dépendances.
