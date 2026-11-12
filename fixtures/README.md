# Fixtures

Données de test enregistrées : flux NMEA, messages AIS, fichiers GPX,
cartes MBTiles de test. Ne contient aucune donnée personnelle.

## Cartes MBTiles de test

Les cartes de test sont générées (non versionnées) par l'exemple
`make_test_chart` de `nav-charts`. Ce sont des grilles colorées
synthétiques : **elles ne doivent jamais servir à la navigation**.

```bash
cargo run -p nav-charts --example make_test_chart -- monde-test.mbtiles "Monde (test)" 0 4 -180,-85,180,85
cargo run -p nav-charts --example make_test_chart -- tahiti-test.mbtiles "Tahiti (test)" 5 11 -149.95,-17.9,-149.1,-17.45
```

## Répertoire des cartes de l'application

NautiK charge les fichiers `.mbtiles` du répertoire :

- `NAUTIK_CHARTS_DIR` si cette variable est définie ;
- sinon `<données de l'application>/charts`
  (Windows : `%APPDATA%\io.github.laurentpellet.nautik\charts`).

Le chemin exact est affiché dans l'application lorsqu'aucune carte
n'est chargée. Les cartes sont lues au démarrage.
