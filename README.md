# Hellaria — mods natifs pour Horizon

Deux mods natifs pour le lanceur Horizon, à utiliser avec le serveur Minecraft Hellaria
(Fabric 1.21.11 + hellaria-bridge). Ils ne servent qu'aux joueurs **Horizon** : les joueurs
Java gardent leurs mods habituels.

| Dossier | Mod | Ce qu'il fait |
|---|---|---|
| `jei/` | JEI for Horizon (version Hellaria) | Liste des objets et recettes, y compris ceux des mods du serveur |
| `hellaria-builder/` | Hellaria Builder Wand | Aperçu des blocs posés, écran de mélange du sac, orientation horizontale / verticale |

## Installer depuis le lanceur

1. Sélectionner l'instance **Horizon**, ouvrir la page **Mods**.
2. Descendre en bas de la page : carte « Add a native mod from GitHub », bouton **Add from GitHub**.
3. Coller le lien de ce dépôt, choisir les mods (ou tous), accepter l'avertissement.
4. Lancer la compilation depuis la page Mods (la première fois : 20 à 30 minutes).

Avant d'installer, désinstaller le JEI officiel : les deux portent l'identifiant `jei`.

En ligne de commande :

    horizon-launcher mods add-github https://github.com/<login>/<depot> --all --trust-native-code

## Compatibilité

Les deux mods sont épinglés sur Horizon Nightly 2026-11-13 (commit `9f42064`). Une autre version
de Horizon peut demander « Compiler quand même » et risque d'échouer.

Le serveur doit avoir `hellaria-bridge` 2.2.0 ou plus ; pour le JEI, le fichier
`config/hellaria-bridge/envoyer-recettes-jei` doit être présent.

Ce sont des mods natifs : ils s'exécutent avec les droits de l'ordinateur. Lire le code avant de les installer.

## Licences

`jei/` : MIT, dérivé de [horizon-jei](https://github.com/enderliker/horizon-jei) et de JustEnoughItems
(voir `jei/LICENSE` et `jei/NOTICE`).
