# Contrôles de base

Ce chapitre présente les contrôles de base de la bibliothèque rust_widgets, y compris les boutons, cases à cocher, étiquettes, etc.

## Bouton (Button)

Le bouton est l'un des contrôles les plus couramment utilisés, utilisé pour déclencher des actions.

```rust
use rust_widgets::create_button;

let button = create_button(parent, "Cliquez-moi", x, y, width, height);
```

## Case à cocher (Checkbox)

La case à cocher est utilisée pour représenter un état binaire (coché ou non coché).

```rust
use rust_widgets::create_checkbox;

let checkbox = create_checkbox(parent, "Option", x, y, width, height);
```

## Étiquette (Label)

L'étiquette est utilisée pour afficher des informations textuelles.

```rust
use rust_widgets::create_label;

let label = create_label(parent, "Ceci est une étiquette", x, y, width, height);
```

## Champ de saisie de texte (LineEdit)

Le champ de saisie de texte est utilisé pour recevoir du texte saisi par l'utilisateur.

```rust
use rust_widgets::create_line_edit;

let line_edit = create_line_edit(parent, "Texte par défaut", x, y, width, height);
```

## Bouton radio (RadioButton)

Le bouton radio est utilisé pour sélectionner une option parmi plusieurs.

```rust
use rust_widgets::create_radio_button;

let radio_button = create_radio_button(parent, "Option 1", x, y, width, height);
```

## Curseur (Slider)

Le curseur est utilisé pour sélectionner une valeur dans une plage donnée.

```rust
use rust_widgets::create_slider;

let slider = create_slider(parent, x, y, width, height);
```

## Barre de progression (ProgressBar)

La barre de progression est utilisée pour afficher la progression d'une opération.

```rust
use rust_widgets::create_progress_bar;

let progress_bar = create_progress_bar(parent, x, y, width, height);
```

## Boîte de sélection (ComboBox)

La boîte de sélection est utilisée pour sélectionner une option dans une liste déroulante.

```rust
use rust_widgets::{create_combo_box, combo_box_add_item};

let combo_box = create_combo_box(parent, x, y, width, height);
combo_box_add_item(combo_box, "Option 1");
combo_box_add_item(combo_box, "Option 2");
```

## Boîte de liste (ListBox)

La boîte de liste est utilisée pour afficher et sélectionner des éléments dans une liste.

```rust
use rust_widgets::{create_list_box, list_box_add_item};

let list_box = create_list_box(parent, x, y, width, height);
list_box_add_item(list_box, "Élément 1");
list_box_add_item(list_box, "Élément 2");
```

## Boîte à chiffres (SpinBox)

La boîte à chiffres est utilisée pour sélectionner une valeur numérique en cliquant sur les boutons haut/bas ou en saisissant directement.

```rust
use rust_widgets::create_spin_box;

let spin_box = create_spin_box(parent, x, y, width, height);
```