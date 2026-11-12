
# NautiK — Plan de tests

## 1. Objectifs

Valider :
- Exactitude des calculs.
- Fiabilité des parseurs.
- Résilience réseau.
- Persistance des données.
- Fonctionnement hors ligne.
- Fiabilité des alarmes.

## 2. Tests unitaires

### Géodésie

- Distance entre deux coordonnées.
- Relèvement initial.
- Passage de l'antiméridien.
- Positions identiques.
- Coordonnées invalides.
- Conversions d'unités.

### NMEA

- Phrase valide.
- Checksum invalide.
- Phrase incomplète.
- Champs absents.
- Valeurs hors limites.
- Talker ID différent.

### AIS

- Message simple.
- Message fragmenté.
- MMSI invalide.
- Coordonnées indisponibles.
- Valeurs réservées.
- Cible obsolète.

### Routes

- Ajout waypoint.
- Suppression waypoint.
- Inversion route.
- Distance totale.
- Export GPX.
- Import GPX invalide.

### Alarmes

- Activation.
- Acquittement.
- Résolution.
- Hystérésis.
- Temporisation.
- Perte de source.
- Redémarrage.

## 3. Tests d'intégration

### Signal K

- Connexion.
- Authentification.
- Abonnement.
- Réception de données.
- Déconnexion.
- Reconnexion.
- Jeton expiré.
- Serveur indisponible.

### SQLite

- Migration initiale.
- Migration successive.
- Contraintes FK.
- Transactions.
- Sauvegarde.
- Restauration.

### MBTiles

- Fichier valide.
- Fichier corrompu.
- Tuile inexistante.
- Métadonnées manquantes.
- Plusieurs cartes.
- Changement de zoom.

## 4. Tests de performance

- 100 cibles AIS.
- 1 000 cibles AIS.
- 10 000 points de trace.
- Plusieurs cartes MBTiles.
- Flux NMEA continu.
- Déconnexion réseau répétée.

Mesurer :
- CPU.
- RAM.
- Latence.
- FPS.
- Temps de démarrage.

## 5. Tests embarqués

- Raspberry Pi ARM64.
- Démarrage automatique.
- Arrêt du service.
- Redémarrage brutal.
- Perte réseau.
- Coupure Signal K.
- Saturation du disque.
- Fonctionnement sans écran.

## 6. Recette MVP

- [ ] Application desktop démarre.
- [ ] Carte MBTiles chargée hors ligne.
- [ ] Position GPS affichée.
- [ ] Signal K connecté.
- [ ] NMEA 0183 décodé.
- [ ] Waypoint créé.
- [ ] Route persistée.
- [ ] Trace enregistrée.
- [ ] AIS affiché.
- [ ] Alarme mouillage fonctionnelle.
- [ ] Service autonome opérationnel.
- [ ] Tests automatisés réussis.

## 7. Conditions de validation

Une phase est terminée lorsque :

1. Le code compile.
2. Les tests passent.
3. Les critères fonctionnels sont satisfaits.
4. La documentation est mise à jour.
5. Aucun défaut bloquant n'est connu.

## 8. Tests en mer

Prévoir des essais progressifs :
- À quai.
- Au mouillage.
- En navigation côtière.
- Avec différentes sources GPS.
- Avec réception AIS réelle.

Les essais doivent être effectués avec
des moyens de navigation indépendants.
