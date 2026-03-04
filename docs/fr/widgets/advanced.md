# Contrôles avancés

Ce chapitre présente les contrôles avancés de la bibliothèque rust_widgets, y compris la vue arborescente, la vue tableau et la vue liste.

## Vue arborescente (TreeView)

La vue arborescente est utilisée pour afficher des données hiérarchiques.

```rust
use rust_widgets::{create_tree_view, tree_view_set_model};
use rust_widgets::widget::VecTreeModel;

let tree_view = create_tree_view(parent, x, y, width, height);

// Créer un modèle arborescent
let mut model = VecTreeModel::new();
model.add_item("Nœud racine", None);
model.add_item("Nœud enfant 1", Some("Nœud racine"));
model.add_item("Nœud enfant 2", Some("Nœud racine"));

// Définir le modèle
tree_view_set_model(tree_view, model);
```

## Vue tableau (TableView)

La vue tableau est utilisée pour afficher des données tabulaires.

```rust
use rust_widgets::{create_table_view, table_view_add_column, table_view_add_row};

let table_view = create_table_view(parent, x, y, width, height);

// Ajouter des colonnes
table_view_add_column(table_view, "Nom", 150);
table_view_add_column(table_view, "Âge", 80);

// Ajouter des lignes
table_view_add_row(table_view, vec!["John Doe", "30"]);
table_view_add_row(table_view, vec!["Jane Smith", "25"]);
```

## Vue liste (ListView)

La vue liste est utilisée pour afficher des données de liste.

```rust
use rust_widgets::{create_list_view, list_view_set_model};
use rust_widgets::widget::VecListModel;

let list_view = create_list_view(parent, x, y, width, height);

// Créer un modèle de liste
let model = VecListModel::new(vec!["Élément 1", "Élément 2", "Élément 3"]);

// Définir le modèle
list_view_set_model(list_view, model);
```