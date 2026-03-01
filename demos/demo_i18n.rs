//! i18n demo.

use rust_widgets::i18n;

fn main() {
    // Load multiple language packs from demo assets.
    i18n::load_translations("demos/assets/i18n_en.json").expect("load i18n file");
    i18n::load_translations("demos/assets/i18n_zh_cn.json").expect("load i18n file");

    // Switch language and print translated values.
    i18n::set_language("en");
    println!("en: {}", i18n::translate("hello"));

    i18n::set_language("zh-CN");
    println!("zh-CN: {}", i18n::translate("hello"));
}
