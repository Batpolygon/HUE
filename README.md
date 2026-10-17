# HUE

**HUE est un compagnon de bureau vivant**, conçu pour Windows et compilable depuis les sources sous Linux. Il prend la forme d’un petit personnage animé qui se déplace sur l’écran, réagit à l’activité du PC et peut discuter avec toi. Son identité et son caractère sont inspirés des composants de la machine sur laquelle il tourne.

HUE est construit avec **Rust**, **Tauri 2**, **TypeScript** et **Vite**. Le projet est développé par [Batpolygon](https://batpolygon.github.io/) et hébergé sur [GitHub](https://github.com/Batpolygon/HUE).

## Télécharger HUE

Télécharge l’EXE Windows depuis la [release officielle HUE](https://github.com/Batpolygon/HUE/releases/tag/OfficialRelease). Cette release fournit uniquement `HUE.exe` : **il n’y a pas d’installateur**. Télécharge l’EXE et lance-le. Pour Linux, compile HUE depuis les sources en suivant les instructions ci-dessous.

Après le téléchargement, lance `HUE.exe` directement : il n’y a ni assistant d’installation ni raccourci créé automatiquement. L’icône de la zone de notification permet d’afficher ou de ranger le compagnon, de redémarrer l’application ou de la quitter.

## Ce que fait HUE

- **Vit sur le bureau** sous la forme d’un personnage animé, sans cadre de fenêtre. Il reste au-dessus des fenêtres ouvertes. Quand il est rangé, il se place sous la barre des tâches.
- **Se déplace selon tes préférences** : patrouille sur l’écran, marche le long de la barre des tâches, ou reste immobile. Tu peux aussi le faire glisser et le lancer.
- **Réagit à l’ordinateur** : HUE observe localement des informations comme le processeur, la mémoire, le disque, la batterie et le réseau. Il peut réagir à certains événements système et actualiser son affichage avec l’heure et des statistiques.
- **A une personnalité liée au matériel** : le profil est dérivé de caractéristiques telles que le processeur, la mémoire vive, la carte graphique et l’espace disque.
- **Peut discuter par texte** avec un modèle compatible avec l’API de chat OpenAI, ou avec un modèle local au format GGUF via `llama.cpp`.
- **Parle avec un son Animalese synthétisé**. Le son, sa hauteur et sa vitesse peuvent être réglés ou désactivés.
- **Peut se cacher automatiquement** lorsque le mode discret est activé et qu’un jeu, OBS ou une fenêtre réellement en plein écran est détecté.
- **Peut démarrer avec ta session Windows**, si tu actives cette option dans les paramètres.

HUE ne propose pas encore la reconnaissance vocale : l’état Whisper affiché par le programme indique que cette fonction n’est pas disponible. Les échanges se font par le champ de texte.

## Utilisation

- Clique sur HUE pour ouvrir son interface de discussion.
- Utilise le bouton des paramètres pour choisir son comportement, sa fréquence de discussion et son moteur de conversation.
- Fais glisser HUE pour le déplacer. Le coin inférieur droit sert à le ranger ou à le faire réapparaître.
- Clique sur l’icône HUE dans la zone de notification pour ouvrir le menu : **Afficher HUE**, **Ranger**, **Redémarrer** ou **Quitter**.

### Réglages disponibles

- **Fréquence des réflexions** : Muet, Discret, Normal, Bavard ou Hyperactif.
- **Déplacement** : patrouille sur l’écran, marche le long de la barre des tâches ou position statique.
- **Affichage quand HUE est rangé** : animation des yeux, horloge, statistiques matérielles ou cycle de ces vues.
- **Mode discret** : masque HUE en plein écran, dans certains jeux détectés et lorsqu’OBS est lancé.
- **LLM** : modèle distant compatible OpenAI ou modèle local GGUF.
- **Animalese** : activation, hauteur et vitesse du son.
- **Démarrage Windows** : lancement automatique à l’ouverture de ta session.

Les paramètres sont enregistrés dans `settings.json`, dans le dossier de configuration utilisateur (sous Windows, généralement `%APPDATA%\hue\settings.json`).

## Configurer les conversations

### API distante

Dans **Paramètres → LLM**, sélectionne **API distante**, puis renseigne l’endpoint, la clé API et le nom du modèle. L’endpoint par défaut est `https://api.openai.com/v1`; les fournisseurs proposant une API compatible avec le format OpenAI peuvent aussi être utilisés.

La clé API est conservée dans le fichier local `settings.json`. Protège ce fichier comme tu protégerais un mot de passe. Les messages sont envoyés au fournisseur choisi lorsque tu utilises ce mode.

### Modèle local GGUF (Windows)

Sélectionne **Local (GGUF)** et indique le chemin vers un fichier modèle `.gguf` que tu as obtenu séparément. Un modèle recommandé est [Meta-Llama-3-8B-Instruct.Q4_K_M.gguf](https://huggingface.co/QuantFactory/Meta-Llama-3-8B-Instruct-GGUF/blob/main/Meta-Llama-3-8B-Instruct.Q4_K_M.gguf) (**environ 4,92 Go**). Télécharge le fichier, puis sélectionne-le dans le champ **Chemin .gguf** des paramètres.

Au premier lancement du moteur local sous Windows, HUE télécharge automatiquement une version CPU Windows de `llama-server` (llama.cpp), puis l’utilise sur `127.0.0.1`. Le téléchargement du moteur nécessite une connexion Internet ; le fichier modèle n’est pas fourni avec HUE. La vitesse et la mémoire nécessaires dépendent du modèle et du matériel.

> Le téléchargement recommandé est un fichier volumineux. Vérifie que tu as assez d’espace disque et consulte la fiche du modèle pour ses conditions de licence. Le démarrage automatique de `llama-server` intégré à HUE est actuellement prévu pour Windows ; sous Linux, utilise le mode API distante.

## Compiler depuis les sources

### Prérequis

- Windows 10 ou 11, ou une distribution Linux prise en charge par Tauri
- [Node.js](https://nodejs.org/) 20 ou plus récent, avec npm
- [Rust stable](https://www.rust-lang.org/tools/install)
- Les bibliothèques système de Tauri pour ton système : [prérequis officiels Tauri](https://v2.tauri.app/start/prerequisites/)

Sur Debian ou Ubuntu, installe les paquets système Tauri avant de compiler :

```bash
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

Sur Windows, il faut également les **Microsoft C++ Build Tools** et **WebView2** ; consulte le [guide Tauri pour Windows](https://v2.tauri.app/start/prerequisites/#windows).

### Lancer en développement

À la racine du dépôt, sous Windows ou Linux :

```bash
npm install
npm run tauri dev
```

### Construire l’application

Pour compiler les ressources web :

```bash
npm run build
```

Sous Windows, pour compiler l’application, copier l’EXE dans `release/` et appliquer son icône :

```powershell
npm run build:exe
```

L’application compilée est copiée dans `release/HUE.exe`. Cette commande ne crée pas d’installateur. Le premier build peut prendre plusieurs minutes et nécessite un accès Internet pour télécharger les dépendances manquantes.

Sous Linux, compile l’application sans créer de paquet d’installation Windows :

```bash
npm run tauri build -- --no-bundle
```

Le binaire Linux compilé se trouve dans `src-tauri/target/release/hue`. Tu peux le lancer depuis un terminal. Pour les autres distributions, installe les dépendances indiquées dans le [guide officiel Tauri](https://v2.tauri.app/start/prerequisites/#linux).

## Confidentialité et fonctionnement

- Les réglages, l’historique de conversation à court terme et le profil matériel sont gérés par l’application sur la machine.
- Les caractéristiques matérielles sont collectées pour créer l’identité de HUE et afficher ses statistiques. Certains événements Windows servent à ses réactions et à la détection du mode discret.
- En mode API distante, les messages de conversation partent vers le fournisseur configuré. En mode GGUF, HUE utilise le serveur local lancé sur `127.0.0.1`.
- Le mode discret peut examiner la fenêtre active et la liste des processus afin de reconnaître les jeux, les logiciels de capture et le plein écran.
- L’option de lancement au démarrage ajoute ou retire une entrée de démarrage dans le profil utilisateur Windows.

## Technologies

- [Tauri 2](https://v2.tauri.app/) et Rust pour l’application native et les fenêtres
- TypeScript et [Vite](https://vite.dev/) pour l’interface
- `sysinfo` pour plusieurs statistiques système
- API de chat compatible OpenAI ou `llama.cpp` pour les conversations

## Structure du projet

```text
src/                  Interface, rendu du compagnon, HUD et son Animalese
src-tauri/src/ai/     Personnalité, conversation, mémoire et moteur local
src-tauri/src/config/ Réglages persistants
src-tauri/src/physics/ Déplacement et physique du compagnon
src-tauri/src/watcher/ Statistiques et événements système
src-tauri/icons/      Icônes de l’application et de la zone de notification
scripts/              Préparation de l’EXE de release
```

## Contribuer

Les contributions et signalements de bugs sont les bienvenus.

## Auteur

**Made by Batpolygon** — projet HUE.
