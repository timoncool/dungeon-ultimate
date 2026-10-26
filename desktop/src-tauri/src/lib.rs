//! Tauri-оболочка Dungeon Ultimate. Ничего тяжёлого сама не делает: поднимает нативный `сервер`
//! (axum, тот же REST/SSE-контракт, что бэкенд-питон) на 127.0.0.1:<свободный порт> и открывает
//! окно на этот URL. Сервер сам раздаёт SPA (frontend/dist) и API на одном origin — фронт работает
//! с относительными путями без правок.
//!
//! Портативность взята из эталона Higgs-Ultimate (desktop/src-tauri/src/lib.rs):
//! app_root_dir = каталог рядом с exe; WEBVIEW2_USER_DATA_FOLDER и рантайм-модели держим там же.

use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use tauri::{WebviewUrl, WebviewWindowBuilder};

/// Каталог рядом с exe (портативная установка). Дев-режим: корень репозитория.
fn app_root_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn resolve_repo_root() -> PathBuf {
    // Явное переопределение (dev / тесты).
    if let Ok(root) = std::env::var("DU_ROOT") {
        return PathBuf::from(root);
    }
    let exe_dir = app_root_dir();
    // Dev: exe в …/desktop/src-tauri/target/<profile>/. Поднимаемся до каталога с crates/.
    let mut dir = exe_dir.as_path();
    for _ in 0..6 {
        if dir.join("crates").is_dir() && dir.join("Cargo.toml").is_file() {
            return dir.to_path_buf();
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }
    if cfg!(windows) {
        return exe_dir;
    }
    // Пакет Linux (/usr/bin, AppImage) и программа macOS только для чтения: данные — в каталоге пользователя.
    package_data_dir().unwrap_or_else(|why| {
        eprintln!("[ERROR] {why}");
        std::process::exit(1);
    })
}

/// Данные установленной игры: на Linux $XDG_DATA_HOME/dungeon-ultimate, иначе ~/.local/share/dungeon-ultimate; на macOS
/// ~/Library/Application Support/Dungeon Ultimate — как у Speech Studio.
fn package_data_dir() -> Result<PathBuf, String> {
    if !cfg!(target_os = "macos") {
        if let Some(root) = std::env::var_os("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
            return Ok(PathBuf::from(root).join("dungeon-ultimate"));
        }
    }
    let home = std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "Neither XDG_DATA_HOME nor HOME is set: there is nowhere to keep the models.".to_string())?;
    let dir = if cfg!(target_os = "macos") {
        home.join("Library").join("Application Support").join("Dungeon Ultimate")
    } else {
        home.join(".local").join("share").join("dungeon-ultimate")
    };
    std::fs::create_dir_all(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    Ok(dir)
}

/// Порт MCP и окна по умолчанию — постоянный, как у студий (YuE2 8791, Dub 8793): по нему агент
/// подключается к игре. `DU_PORT` его меняет. Занят — берётся свободный, и панель «Активность»
/// показывает фактический адрес MCP.
const DEFAULT_PORT: u16 = 8795;

fn pick_port() -> std::io::Result<u16> {
    let wanted = std::env::var("DU_PORT").ok().and_then(|value| value.parse().ok()).unwrap_or(DEFAULT_PORT);
    if TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, wanted)).is_ok() {
        return Ok(wanted);
    }
    eprintln!("[ERROR] порт {wanted} занят — игра слушает другой; адрес MCP показан в панели «Активность»");
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))?;
    Ok(listener.local_addr()?.port())
}

/// Дождаться, пока сервер начнёт принимать соединения (или таймаут).
fn wait_until_ready(port: u16, timeout: Duration) -> bool {
    let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
    let start = Instant::now();
    while start.elapsed() < timeout {
        if TcpStream::connect_timeout(&addr.into(), Duration::from_millis(200)).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(120));
    }
    false
}

/// Спрятать ОКНО консоли. Exe — console-subsystem (чтобы дети ffmpeg/llama/roformer наследовали ОДНУ
/// консоль и НЕ плодили своих окон на каждый спавн). Сама консоль остаётся выделенной (дети её наследуют),
/// прячем только её ОКНО -> исчезает чёрное окно и его пустая запись в ALT+TAB/панели задач; в переключателе
/// остаётся лишь GUI-окно (с иконкой, см. .icon() ниже). Дети по-прежнему не открывают окон.
#[cfg(windows)]
fn hide_console_window() {
    extern "system" {
        fn GetConsoleWindow() -> isize;
        fn ShowWindow(hwnd: isize, n_cmd_show: i32) -> i32;
        fn GetConsoleProcessList(lpdw_process_list: *mut u32, dw_process_count: u32) -> u32;
    }
    unsafe {
        // Прячем ТОЛЬКО собственную консоль. Если exe запущен ИЗ существующего терминала, наш процесс
        // делит его консоль (к ней привязано >1 процесса) — это ЧУЖОЕ окно терминала пользователя, трогать
        // нельзя. При двойном клике из Проводника загрузчик создаёт нам отдельную консоль (count==1).
        let mut pids = [0u32; 4];
        let n = GetConsoleProcessList(pids.as_mut_ptr(), pids.len() as u32);
        if n != 1 {
            return;
        }
        let hwnd = GetConsoleWindow();
        if hwnd != 0 {
            ShowWindow(hwnd, 0); // SW_HIDE
        }
    }
}

/// Проверка обновления на GitHub-релизе и, по согласию игрока, установка.
///
/// Ведём из Rust, а не из окна: фронт грузится с внешнего http-адреса встроенного сервера, где
/// JS-мост Tauri ненадёжен, а апдейтер на стороне Rust работает независимо от webview. Нет сети
/// или обновления — выходим молча. Портативную раскладку на лету не обновляем: перезаписать
/// каталог, из которого прямо сейчас запущена игра, нельзя — предлагаем открыть страницу.
const RELEASES_URL: &str = "https://github.com/timoncool/dungeon-ultimate/releases/latest";

fn open_in_browser(app: &tauri::AppHandle, url: &tauri::Url) {
    use tauri_plugin_opener::OpenerExt;
    if let Err(e) = app.opener().open_url(url.as_str(), None::<&str>) {
        eprintln!("[ERROR] ссылка {url} не открылась в браузере: {e}");
    }
}

fn spawn_update_check(app: tauri::AppHandle) {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
    use tauri_plugin_updater::UpdaterExt;
    tauri::async_runtime::spawn(async move {
        let Ok(updater) = app.updater() else { return };
        let update = match updater.check().await {
            Ok(Some(update)) => update,
            _ => return,
        };
        let version = update.version.clone();
        // Окно сообщает свой язык вскоре после загрузки: вопрос ждёт его.
        let _ = tauri::async_runtime::spawn_blocking(|| {
            for _ in 0..100 {
                if du_server::ui_lang::reported() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        })
        .await;
        use du_server::ui_lang::text;
        let yes = app
            .dialog()
            .message(text("update-install-now", &[("version", &version)]))
            .title(text("update-title", &[]))
            .kind(MessageDialogKind::Info)
            .buttons(MessageDialogButtons::OkCancelCustom(text("update-button-install", &[]), text("update-button-later", &[])))
            .blocking_show();
        if !yes {
            return;
        }
        match update.download_and_install(|_, _| {}, || {}).await {
            Ok(_) => app.restart(),
            Err(error) => {
                // Не вышло само — не оставляем игрока ни с чем: предлагаем страницу релиза.
                use tauri_plugin_opener::OpenerExt;
                let open = app
                    .dialog()
                    .message(text("update-failed", &[("error", &error.to_string())]))
                    .title(text("update-title", &[]))
                    .kind(MessageDialogKind::Error)
                    .buttons(MessageDialogButtons::OkCancelCustom(text("update-button-open", &[]), text("update-button-later", &[])))
                    .blocking_show();
                if open {
                    let _ = app.opener().open_url(RELEASES_URL, None::<&str>);
                }
            }
        }
    });
}

pub fn run() {
    #[cfg(windows)]
    hide_console_window();
    // Портатив: состояние WebView2 (localStorage) держим рядом с exe, а не в профиле пользователя.
    if std::env::var_os("WEBVIEW2_USER_DATA_FOLDER").is_none() {
        std::env::set_var(
            "WEBVIEW2_USER_DATA_FOLDER",
            app_root_dir().join("webview-data"),
        );
    }

    du_server::diagnostics::init_logging();
    let repo_root = resolve_repo_root();
    du_server::diagnostics::log_to_file(&repo_root);
    let port = pick_port().expect("нет свободного порта на 127.0.0.1");

    // axum-бэкенд поднимается В ЭТОМ ЖЕ процессе на фоновом потоке — ОДИН exe, без сервер.exe-сайдкара.
    // Поток-демон: живёт до выхода процесса, отдельно убивать не нужно (нет дочернего процесса).
    let root = repo_root.clone();
    std::thread::spawn(move || {
        if let Err(e) = du_server::serve_blocking(&root, port) {
            eprintln!("встроенный сервер упал: {e}");
        }
    });

    // Ждём готовности сервера, чтобы окно не открылось на пустоту.
    if !wait_until_ready(port, Duration::from_secs(30)) {
        eprintln!("встроенный сервер не поднялся на 127.0.0.1:{port} за 30с");
    }

    let url = format!("http://127.0.0.1:{port}/");

    tauri::Builder::default()
        // Скрипт ссылок плагина отвечает на клик через IPC, которого у страницы с http://127.0.0.1 нет;
        // без него клик доходит до on_new_window и уходит в браузер.
        .plugin(tauri_plugin_opener::Builder::new().open_js_links_on_click(false).build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(move |app| {
            // Иконка бандла для GUI-окна: без явной установки окно оставалось пустым в ALT+TAB/панели задач
            // (иконка висела на консольном окне). Ставим её на само GUI-окно.
            let icon = app.default_window_icon().cloned();
            let (for_tabs, for_links) = (app.handle().clone(), app.handle().clone());
            let win = WebviewWindowBuilder::new(
                app,
                "main",
                WebviewUrl::External(url.parse().expect("валидный URL")),
            )
            .title(format!("Dungeon Ultimate {}", app.package_info().version))
            .inner_size(1600.0, 1040.0)
            .min_inner_size(1280.0, 800.0)
            .resizable(true)
            // Tauri v2 по умолчанию перехватывает OS-drop файлов -> HTML5 onDrop в дропзоне НЕ срабатывает
            // (юзеры жаловались «перетаскивание не работает»). Отключаем перехват -> webview сам ловит drop.
            .disable_drag_drop_handler()
            // WebView2 оставляет запрос микрофона без ответа и без окна согласия: голосовой ввод своей страницы
            // висел на getUserMedia
            .on_permission_request(move |webview, kind| match kind {
                tauri::webview::PermissionKind::Microphone
                    if webview.url().is_ok_and(|url| url.host_str() == Some("127.0.0.1") && url.port() == Some(port)) =>
                {
                    tauri::webview::PermissionResponse::Allow
                }
                _ => tauri::webview::PermissionResponse::Default,
            })
            // Озвучка играет САМА, без предварительного клика по странице: WebView2 иначе
            // отклоняет `audio.play()` — фразы синтезируются, а игрок сидит в тишине.
            // Стандартные аргументы wry перечисляем заново: своя строка их затирает.
            .additional_browser_args(
                "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection \
                 --autoplay-policy=no-user-gesture-required",
            )
            // WebView2 молча глотает target="_blank" и window.open, если у окна нет обработчика новых окон.
            .on_new_window(move |target, _features| {
                open_in_browser(&for_tabs, &target);
                tauri::webview::NewWindowResponse::Deny
            })
            .on_navigation(move |target| {
                let own = target.host_str() == Some("127.0.0.1") && target.port() == Some(port);
                if own || !matches!(target.scheme(), "http" | "https") {
                    return true;
                }
                open_in_browser(&for_links, target);
                false
            })
            .build()?;
            // Иконка окна (ALT+TAB/таскбар) — ПОСЛЕ создания: не паникуем, если не выйдет, окно рабочее.
            if let Some(ic) = icon {
                let _ = win.set_icon(ic);
            }
            // Новая версия проверяется в фоне и ставится по согласию игрока.
            spawn_update_check(app.handle().clone());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("ошибка запуска Tauri")
        .run(|_, event| {
            if let tauri::RunEvent::Exit = event {
                du_server::hub::flush_on_exit();
            }
        });
}
