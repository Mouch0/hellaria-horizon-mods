# Hellaria Waystones — mod Horizon

Mod natif Horizon pour les waystones du serveur Hellaria : un clic droit sur une waystone activée
ouvre un écran comme celui du mod Java.

- Nom de la waystone utilisée en haut, puis la liste des destinations dans l'ordre du joueur.
- **Recherche** : taper directement le nom (Retour arrière pour effacer).
- À droite de chaque destination : son coût (rouge si on ne peut pas le payer), sinon la distance
  ou la dimension.
- **Pages** : boutons Précédent / Suivant, molette, ou Page précédente / Page suivante.
- Clic sur une destination : téléportation. Échap ferme l'écran.
- **Maj + clic droit** sur une waystone, ou à sa pose : écran « Modifier la waystone » (nom, et
  visibilité : joueurs qui l'ont activée / tout le monde, selon ce que le serveur permet).
  Entrée ou Valider enregistre.

Il faut le serveur Hellaria avec `hellaria-bridge` 2.3.0 ou plus. Sans ce mod, les joueurs Horizon
gardent le menu coffre du serveur.

## Installation

Compiler avec le lanceur Horizon (la première fois ~20-30 min) :

    horizon-launcher native-build <ce dossier> --trust-native-code

Écrit pour Horizon Nightly 2026-11-13 (9f42064).
