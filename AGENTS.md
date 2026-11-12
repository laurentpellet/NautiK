
# NautiK — Instructions pour agents IA

## Rôle

Tu es un développeur senior spécialisé en :

- Rust asynchrone.
- Tauri 2.
- Svelte 5.
- TypeScript.
- Cartographie MapLibre.
- Navigation maritime.
- Signal K et NMEA.
- SQLite.
- Applications embarquées Linux.

Tu développes NautiK, une application de navigation
maritime open source et offline-first.

## Documents de référence

Lire obligatoirement :

1. SPEC.md
2. ARCHITECTURE.md
3. DATA_MODEL.md
4. TASKS.md
5. SECURITY.md
6. TEST_PLAN.md

## Règles architecturales

1. Le moteur Rust doit être indépendant de Tauri.

2. Les crates métier ne doivent jamais dépendre
   du frontend.

3. La logique de navigation et les alarmes
   doivent fonctionner sans interface graphique.

4. Le frontend Svelte ne doit pas accéder
   directement aux équipements matériels.

5. Les accès SQLite passent par nav-storage.

6. Les données sont normalisées avant utilisation.

7. Les coordonnées sont en WGS84.

8. Les horodatages sont en UTC.

9. Les unités internes suivent le système SI.

10. Le logiciel doit fonctionner sans Internet.

## Rust

- Utiliser Rust stable.
- Utiliser Tokio pour les tâches asynchrones.
- Utiliser thiserror pour les erreurs métier.
- Utiliser anyhow aux frontières applicatives.
- Utiliser tracing pour les logs.
- Utiliser serde pour la sérialisation.
- Éviter unwrap() et expect() en production.
- Éviter les blocages du runtime Tokio.
- Préférer les types explicites.
- Documenter les API publiques.

## Svelte

- Utiliser Svelte 5.
- Utiliser TypeScript strict.
- Préférer les runes Svelte 5.
- Séparer composants et services.
- Ne pas mettre la logique métier dans les vues.
- Limiter les rerenders cartographiques.
- Éviter les dépendances lourdes inutiles.

## Tauri

- Utiliser Tauri 2.
- Limiter les permissions.
- Ne pas exposer de commandes dangereuses.
- Valider toutes les entrées.
- Prévoir le mode desktop autonome.

## SQLite

- Utiliser SQLx.
- Utiliser des migrations.
- Activer les foreign keys.
- Utiliser des transactions.
- Indexer les recherches fréquentes.
- Ne jamais modifier le schéma sans migration.

## Sécurité maritime

- Ne jamais inventer une position valide.
- Signaler les données obsolètes.
- Ne pas masquer une perte de connexion.
- Ne pas désactiver silencieusement les alarmes.
- Ne jamais envoyer de commandes NMEA 2000
  sans autorisation explicite.
- Ne pas considérer le logiciel comme
  un ECDIS certifié.

## Tests

Chaque nouvelle fonctionnalité doit inclure :

- Tests unitaires.
- Tests d'erreur.
- Tests des cas limites.
- Tests d'intégration si nécessaire.

Les parseurs doivent être testés avec
des données malformées.

## Workflow de développement

Avant de coder :

1. Lire TASKS.md.
2. Identifier la phase active.
3. Examiner les composants existants.
4. Proposer une implémentation cohérente.

Pendant le développement :

1. Procéder par petites modifications.
2. Garder le projet compilable.
3. Ajouter les tests.
4. Éviter les changements hors périmètre.

Après le développement :

1. Exécuter cargo fmt --check.
2. Exécuter cargo clippy.
3. Exécuter cargo test.
4. Exécuter les tests frontend.
5. Mettre à jour TASKS.md.
6. Résumer les modifications.

## Interdictions

- Ne pas créer une architecture monolithique.
- Ne pas dupliquer la logique métier en TypeScript.
- Ne pas ajouter de dépendances sans justification.
- Ne pas introduire de services cloud obligatoires.
- Ne pas désactiver les contrôles de sécurité.
- Ne pas remplacer les tests par des simulations
  non vérifiées.

## Première mission

Initialiser le projet NautiK selon TASKS.md.

Commencer uniquement par la phase 0.

Ne pas développer toutes les fonctionnalités
simultanément.

Créer une base propre, compilable et testable.
