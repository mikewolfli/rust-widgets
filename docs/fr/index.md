# Documentation Rust Widgets

Bienvenue dans la documentation **rust_widgets** ! Il s'agit d'un guide complet pour créer des applications GUI multiplateformes avec le framework rust_widgets.

## Qu'est-ce que rust_widgets ?

**rust_widgets** est un puissant framework GUI multiplateforme pour Rust qui vous permet de créer des applications natives pour ordinateurs de bureau et mobiles. Il offre :

- **Multiplateforme** : Écrivez une fois, exécutez sur Windows, macOS, Linux, HarmonyOS et systèmes embarqués
- **Performance native** : Intégration directe à la plateforme sans surcharge de virtualisation
- **Ensemble riche de widgets** : Collection complète de composants UI basiques et avancés
- **Support multilingue** : Internationalisation (i18n) intégrée avec changement de langue à l'exécution
- **Liaisons multi-langages** : Utilisable depuis Rust, C, Python, Java et plus encore
- **Accélération GPU** : Backend de rendu GPU optionnel pour des graphismes haute performance

## Liens rapides

- [Démarrage rapide](getting-started/quickstart.md) - Commencez à créer votre première application
- [Référence des widgets](widgets/basic.md) - Parcourez les composants UI disponibles
- [Documentation API](api/rust.md) - Explorez la référence API
- [Démos](demos/basic.md) - Voyez des exemples d'applications

## Exemple

Voici une simple application "Hello World" :

```rust
use rust_widgets::{create_window, create_label, show_widget, run, init};

fn main() {
    init();
    
    let window = create_window("Hello World", 100, 100, 400, 300);
    let label = create_label(window, "Hello, rust_widgets!", 20, 20, 200, 30);
    
    show_widget(window);
    run();
}
```

## Fonctionnalités

### Types de widgets

- **Widgets de base** : Bouton, Étiquette, Édition de ligne, Édition de texte, Case à cocher, Bouton radio, Boîte combo, Curseur, Barre de progression, Boîte de rotation
- **Widgets conteneurs** : Panneau, Séparateur, Widget à onglets, Panneau d'ancrage, Zone de défilement, Boîte de groupe
- **Widgets avancés** : Vue arborescente, Vue tabulaire, Vue liste, Zone MDI, Graphique, Canevas
- **Widgets de dialogue** : Boîte de message, Dialogue de fichier, Dialogue de saisie, Dialogues personnalisés

### Support des plateformes

| Plateforme | Statut | Notes |
|------------|--------|-------|
| Windows | ✅ Supporté | API Win32 avec prise en charge DPI |
| macOS | ✅ Supporté | Intégration Cocoa/AppKit |
| Linux | ✅ Supporté | Backends GTK3/GTK4 |
| HarmonyOS | ✅ Supporté | Intégration native ArkUI |
| Embarqué | ✅ Supporté | Moteur de rendu personnalisé |

### Liaisons de langage

- **Rust** : API native (principale)
- **C** : Compatibilité ABI complète
- **Python** : Wrapper Pythonic avec annotations de type
- **Java** : Intégration basée sur JNI
- **C++** : Wrapper header-only

## Architecture

rust_widgets suit une architecture en couches :

```
┌─────────────────────────────────────┐
│        Couche Application           │
│    (Votre application GUI)          │
├─────────────────────────────────────┤
│        Couche Widget                │
│    (Boutons, Étiquettes, etc.)      │
├─────────────────────────────────────┤
│        Abstraction de plateforme    │
│    (Interface multiplateforme)      │
├─────────────────────────────────────┤
│        Backends natifs              │
│  (Win32/Cocoa/GTK/ArkUI)            │
└─────────────────────────────────────┘
```

## Obtenir de l'aide

- **Problèmes** : Signalez des bugs sur [GitHub Issues](https://github.com/your-org/rust-widgets/issues)
- **Discussions** : Rejoignez les discussions communautaires
- **Documentation** : Parcourez ce guide complet

## Licence

rust_widgets est sous licence MIT. Voir [LICENSE](../LICENSE) pour plus de détails.

---

**Prêt à commencer ?** Rendez-vous au [Guide de démarrage rapide](getting-started/quickstart.md) !
