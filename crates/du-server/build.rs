//! Гарантирует, что каталог собранного интерфейса существует к моменту компиляции.
//!
//! Интерфейс вшивается в бинарь (см. `spa::Ui`), а вшивать можно только существующий
//! каталог. В репозитории `frontend/dist` не хранится — он появляется после сборки
//! интерфейса. На свежем клоне, где интерфейс ещё не собирали, компиляция без этой
//! заглушки просто падала бы, хотя собрать сервер отдельно — совершенно законно.

use std::path::PathBuf;

fn main() {
    let dist = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("frontend")
        .join("dist");
    println!("cargo:rerun-if-changed={}", dist.display());

    if dist.join("index.html").is_file() {
        return;
    }
    if std::fs::create_dir_all(&dist).is_err() {
        return;
    }
    // Заглушка честно говорит, что произошло: пустая страница выглядела бы поломкой.
    let _ = std::fs::write(
        dist.join("index.html"),
        "<!doctype html><meta charset=\"utf-8\">\
         <title>Dungeon Ultimate</title>\
         <body style=\"font:16px/1.5 system-ui;padding:2rem\">\
         Интерфейс не собран. Выполни <code>cd frontend &amp;&amp; yarn build</code> \
         и пересобери сервер.</body>",
    );
}
