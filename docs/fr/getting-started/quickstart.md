# Guide de démarrage rapide

Ce guide vous aidera à démarrer avec rust_widgets en quelques minutes.

## Prérequis

Avant de commencer, assurez-vous d'avoir :

- **Rust** (1.70 ou ultérieur) installé - [Installer Rust](https://rustup.rs/)
- Un compilateur C pour votre plateforme (généralement inclus avec Rust)

## Installation

Ajoutez rust_widgets à votre `Cargo.toml` :

```toml
[dependencies]
rust_widgets = "0.5"
```

Ou utilisez cargo add :

```bash
cargo add rust_widgets
```

## Votre première application

Créez un nouveau projet Rust :

```bash
cargo new my_first_app
cd my_first_app
```

Modifiez `src/main.rs` :

```rust
use rust_widgets::{
    create_window, create_label, create_button, show_widget, run, init,
    connect_clicked, set_widget_text
};

fn main() {
    // Initialiser le framework
    init();
    
    // Créer la fenêtre principale
    let window = create_window("My First App", 100, 100, 400, 300);
    
    // Créer une étiquette
    let label = create_label(window, "Hello, rust_widgets!", 20, 20, 200, 30);
    
    // Créer un bouton
    let button = create_button(window, "Click Me!", 20, 60, 100, 30);
    
    // Connecter l'événement de clic du bouton
    connect_clicked(button, move || {
        set_widget_text(label, "Button clicked!");
    });
    
    // Afficher la fenêtre et démarrer la boucle d'événements
    show_widget(window);
    run();
}
```

Exécutez votre application :

```bash
cargo run
```

## Prochaines étapes

- Apprenez-en plus sur les [Widgets de base](../widgets/basic.md)
- Explorez la [Gestion des événements](../concepts/events.md)
- Consultez les [Démos](../demos/basic.md)
- Lisez la [Vue d'ensemble de l'architecture](../concepts/architecture.md)

## Dépannage

### Erreurs de compilation

Si vous rencontrez des erreurs de compilation :

1. Assurez-vous que votre version de Rust est à jour : `rustup update`
2. Vérifiez que vous avez installé les bibliothèques système requises
3. Consultez les notes spécifiques à la plateforme dans [Installation](installation.md)

### Problèmes d'exécution

Si l'application ne démarre pas :

1. Vérifiez que votre environnement d'affichage est correctement configuré
2. Sur Linux, assurez-vous d'avoir installé les bibliothèques de développement GTK
3. Sur Windows, assurez-vous d'avoir installé le Windows SDK

## Obtenir de l'aide

- Parcourez la [FAQ](../appendix/faq.md)
- Consultez [GitHub Issues](https://github.com/your-org/rust-widgets/issues)
- Rejoignez nos discussions communautaires
