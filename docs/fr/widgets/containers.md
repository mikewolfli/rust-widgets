# Contrôles conteneurs

Ce chapitre présente les contrôles conteneurs de la bibliothèque rust_widgets, y compris les panneaux, onglets, séparateurs, etc.

## Panneau (Panel)

Le panneau est le contrôle conteneur le plus basique, utilisé pour organiser d'autres contrôles.

```rust
use rust_widgets::create_panel;

let panel = create_panel(parent, x, y, width, height);
```

## Contrôle d'onglets (TabWidget)

Le contrôle d'onglets permet à l'utilisateur de basculer entre différentes fenêtres à l'aide d'onglets.

```rust
use rust_widgets::{create_tab_widget, tab_widget_add_tab};

let tab_widget = create_tab_widget(parent, x, y, width, height);
tab_widget_add_tab(tab_widget, "Onglet 1");
tab_widget_add_tab(tab_widget, "Onglet 2");
```

## Sépareur (Splitter)

Le séparateur permet à l'utilisateur de redimensionner les contrôles enfants en faisant glisser une barre de séparation.

```rust
use rust_widgets::{create_splitter, splitter_add_child};

// Créer un séparateur horizontal
let splitter = create_splitter(parent, true, x, y, width, height);

// Ajouter un contrôle enfant
splitter_add_child(splitter, child_widget, 50); // 50% de l'espace
```

## Panneau d'ancrage (DockPanel)

Le panneau d'ancrage permet à l'utilisateur d'ancrer des contrôles enfants sur différents bords du conteneur.

```rust
use rust_widgets::{create_dock_panel, dock_panel_dock_widget, DockPosition};

let dock_panel = create_dock_panel(parent, x, y, width, height);
dock_panel_dock_widget(dock_panel, widget, DockPosition::Left, "Panneau gauche");
```

## Zone MDI (MdiArea)

La zone MDI permet à l'utilisateur de gérer plusieurs fenêtres de document dans une seule fenêtre parente.

```rust
use rust_widgets::{create_mdi_area, mdi_area_add_subwindow};

let mdi_area = create_mdi_area(parent, x, y, width, height);
let subwindow = mdi_area_add_subwindow(mdi_area, "Document 1", 100, 100, 400, 300);
```