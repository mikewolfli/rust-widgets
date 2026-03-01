# rust_widgets Aide (Français)

## Documents liés

- Architecture : [ARCHITECTURE.md](ARCHITECTURE.md)
- Catalogue des démos : [../demos/README.md](../demos/README.md)
- Aide en anglais : [HELP.en.md](HELP.en.md)
- Aide en chinois simplifié : [HELP.zh-CN.md](HELP.zh-CN.md)
- Aide en chinois traditionnel : [HELP.zh-TW.md](HELP.zh-TW.md)
- Aide en russe : [HELP.ru.md](HELP.ru.md)
- Démarrage rapide C ABI : [C_ABI_QUICKSTART.md](C_ABI_QUICKSTART.md)
- Pont natif Harmony : [HARMONY_NATIVE_BRIDGE.fr.md](HARMONY_NATIVE_BRIDGE.fr.md)

## Résumé

- Architecture GUI native multiplateforme en Rust pur.
- Cibles bureau : Windows, macOS, Linux, Harmony Desktop.
- Profil embarqué allégé pour une empreinte minimale.
- API unifiée réservée pour mobile (Android / iOS / Harmony mobile).
- Modules : file d’événements, signaux-slots, thèmes/styles, layouts, XML, i18n, impression, PDF, graphiques.

## Profils

- Complet : features `default` + `full`.
- Allégé : feature `embedded`.
- Réservation mobile : feature `mobile-api` pour les points d’extension mobiles unifiés.

## Commandes

```bash
cargo check
cargo check --examples
cargo run --example demo_main
```

## Exemples de bascule de fonctionnalités

```bash
# Profil complet (par défaut)
cargo check

# Profil embarqué allégé
cargo check --no-default-features --features embedded

# Profil complet + réservation API mobile
cargo check --features "full,mobile-api"

# Profil embarqué + réservation API mobile
cargo check --no-default-features --features "embedded,mobile-api"
```

## Workflow de publication v3

```bash
# smoke tests des démos (default + embedded)
tools/smoke_demos.sh

# validation de publication sans upload
cargo publish --dry-run
```

## Démos

- Liste complète catégorisée : `demos/README.md`.
- Démos d’architecture : `demo_main`, `demo_layout`, `demo_xml`, `demo_i18n`.
- Démo de polling natif : `demo_native_events` (déclenchements menu + widget typé).
- Les démos de contrôles couvrent fenêtre/dialogue/popup, saisie de base, affichage de données,
  conteneurs, menu/outil/statut, ainsi que les contrôles table/grille/graphique/canvas.

## Liaison multi-langage

L’ABI C est définie dans `src/bindings/mod.rs`, avec points d’extension réservés pour Python/C++/Java.
Elle expose aussi les APIs de polling natif : `rust_widgets_poll_menu_triggered` et `rust_widgets_poll_widget_triggered`.
Pour un événement widget typé, utilisez `rust_widgets_poll_widget_trigger_event(widget_id_out)` avec codes : `0` aucun, `1` clic, `2` changement de valeur.
La qualité de rendu se configure via l’ABI C avec `rust_widgets_set_render_aa_samples_per_axis` / `rust_widgets_get_render_aa_samples_per_axis` (valeur bornée à `1..=8`).
Pour le câblage direct ArkUI/NAPI sur Harmony, utilisez les entrées `rust_widgets_harmony_on_*` et `rust_widgets_harmony_on_node_*`.
Le flux `node_handle ↔ widget_id` et l’exemple d’intégration sont décrits dans `docs/HARMONY_NATIVE_BRIDGE.fr.md` et `examples/harmony_napi_bridge_sample.c`.
Pour les commandes complètes de build/run C ABI, consultez `docs/C_ABI_QUICKSTART.md`.

Compilation/exécution rapide (depuis la racine du projet) :

```bash
# Compiler la bibliothèque dynamique
cargo build

# Compiler l’exemple C sur macOS
clang -Iexamples examples/c_abi_poll_demo.c -Ltarget/debug -lrust_widgets -o target/debug/c_abi_poll_demo

# Exécuter sur macOS
DYLD_LIBRARY_PATH=target/debug ./target/debug/c_abi_poll_demo
```

Exemple Linux (chargeur runtime) :

```bash
LD_LIBRARY_PATH=target/debug ./target/debug/c_abi_poll_demo
```
