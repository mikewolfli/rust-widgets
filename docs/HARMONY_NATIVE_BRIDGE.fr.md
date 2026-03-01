# Pont natif Harmony (ArkUI/NAPI)

Utilisez ce guide pour connecter les callbacks ArkUI/NAPI Harmony à `rust_widgets` avec la même structure en étapes que le guide anglais.

## Objectif

Permettre aux callbacks ArkUI/NAPI d’alimenter le même pipeline de polling menu/widget que les backends desktop.

## Étape 1 : Build et initialisation

Feature optionnelle :

```bash
cargo check --features harmony-native
```

Séquence de démarrage recommandée :

1. Appeler `rust_widgets_init()`.
2. Créer la fenêtre et les contrôles via `rust_widgets_create_*`.
3. Conserver les `widget_id` retournés dans l’état natif.

## Étape 2 : Liaison des node handles ArkUI

Quand un node est créé et associé à un `widget_id`, liez une fois :

- `rust_widgets_harmony_bind_node(node_handle, widget_id)`

Quand un node est détruit :

- `rust_widgets_harmony_unbind_node(node_handle)`

Au teardown :

- `rust_widgets_harmony_clear_node_bindings()`

## Étape 3 : Forward des callbacks ArkUI/NAPI

À appeler depuis les callbacks ArkUI/NAPI :

- `rust_widgets_harmony_on_menu_item(menu_item_id)`
- `rust_widgets_harmony_on_click(widget_id)`
- `rust_widgets_harmony_on_value_changed(widget_id)`
- `rust_widgets_harmony_on_widget_event(widget_id, kind_code)`

Alias basés sur node handle :

- `rust_widgets_harmony_on_node_menu_item(node_handle)`
- `rust_widgets_harmony_on_node_click(node_handle)`
- `rust_widgets_harmony_on_node_value_changed(node_handle)`
- `rust_widgets_harmony_on_node_widget_event(node_handle, kind_code)`

API de lookup optionnelle :

- `rust_widgets_harmony_lookup_widget_id(node_handle)`

## Étape 4 : Polling et dispatch dans la boucle applicative

Consommez les événements queue à chaque tick :

- `rust_widgets_poll_menu_triggered()`
- `rust_widgets_poll_widget_trigger_event(widget_id_out)`

À ce stade, aucun thread de bridge supplémentaire n’est requis : les callbacks enqueue directement les événements.

## Étape 5 : Mapping des triggers et APIs fallback

Codes de trigger :

- `1` : clicked
- `2` : value-changed
- autre : unknown

APIs fallback génériques :

- `rust_widgets_inject_menu_trigger(menu_item_id)`
- `rust_widgets_inject_widget_trigger_event(widget_id, kind_code)`

## Fichiers liés

Ressources de référence :

- `examples/harmony_napi_bridge_sample.c`
- `examples/harmony_napi_bridge_flow.md`
