//! Redirector - A URL redirector/cleaner for Linux desktop.
//!
//! Acts as a browser replacement that intercepts URLs, cleans them via
//! ClearURLs rules, applies regex patterns, and optionally opens them
//! automatically via automation rules.
//!
//! Usage:
//!   redirector --url <URL>          # Process a single URL and show dialog
//!   redirector --url <URL> --json   # Output cleaned URL as JSON
//!   redirector --url <URL> --print  # Print cleaned URL to stdout

mod automations;
mod config;
mod modules;
mod pipeline;
mod url_data;

use clap::Parser;
use tracing::info;

/// Represents a registered browser found via `gio mime`.
#[derive(Debug, Clone)]
struct Browser {
    desktop_id: String,
    name: String,
}

/// Find all registered browsers by parsing `gio mime x-scheme-handler/http` output,
/// then resolving each desktop ID to a display name via `gio info`.
fn find_registered_browsers() -> Vec<Browser> {
    let output = std::process::Command::new("gio")
        .args(["mime", "x-scheme-handler/http"])
        .output();

    let Some(output) = output.ok() else {
        return Vec::new();
    };
    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut browsers = Vec::new();
    let mut in_registered = false;

    for line in stdout.lines() {
        if line.trim() == "Registered applications:" {
            in_registered = true;
            continue;
        }
        if line.trim() == "Recommended applications:" {
            break;
        }
        if in_registered {
            let desktop_id = line.trim().trim_end_matches('.').to_string();
            if desktop_id.is_empty()
                || desktop_id == "Default application"
                || desktop_id == "redirector.desktop"
            {
                continue;
            }
            if let Some(name) = get_desktop_name(&desktop_id) {
                browsers.push(Browser {
                    desktop_id,
                    name,
                });
            }
        }
    }

    browsers
}

/// Get the display name of a .desktop file using `gio info`.
fn get_desktop_name(desktop_id: &str) -> Option<String> {
    let output = std::process::Command::new("gio")
        .args(["info", "-a", "text.description", desktop_id])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    // gio info outputs "text::description: Browser Name\n" or similar
    for line in stdout.lines() {
        if let Some(name) = line.strip_prefix("text::description: ") {
            let name = name.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }

    // Fallback: use the desktop ID as the name
    Some(desktop_id.trim_end_matches(".desktop").to_string())
}

#[derive(Parser, Debug)]
#[command(name = "redirector", version, about)]
struct Args {
    /// URL to process
    #[arg(short, long, required = true)]
    url: String,

    /// Output as JSON (for pipe/automation use)
    #[arg(long, default_value_t = false)]
    json: bool,

    /// Print result to stdout (no GUI)
    #[arg(long, default_value_t = false)]
    print: bool,
}

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "redirector=debug,pipeline=info".into()),
        )
        .init();

    let args = Args::parse();

    info!("Processing URL: {}", args.url);

    // Build the pipeline with default modules
    let pipeline = pipeline::Pipeline::default();

    // Run the pipeline (ClearURLs, patterns, etc.)
    let result = pipeline.process(&args.url);

    // Check automations against the FINAL (processed) URL
    if let Some((name, rule)) = automations::execute_automations(&result.url) {
        info!("Automation '{name}' matched: action={}", rule.action);
        if rule.action == "open" {
            if let Some(ref browser) = rule.browser {
                let _ = open_via_gio(browser, &result.url);
            } else {
                let _ = open_url_default(&result.url);
            }
            return;
        }
    }

    // Output based on mode
    if args.json {
        println!(
            "{}",
            serde_json::json!({
                "url": result.url,
                "changes": result.changes,
                "extra": result.extra
            })
        );
    } else if args.print {
        println!("{}", result.url);
    } else {
        // GUI mode - show dialog
        show_dialog(&args.url, &result);
    }
}

fn show_dialog(original: &str, result: &pipeline::PipelineResult) {
    use gtk4::{
        prelude::*, Align, Box as BoxContainer, Button, Label, Orientation,
    };
    use glib::clone;
    use libadwaita::prelude::*;

    let _ = gtk4::init();
    libadwaita::init().unwrap();

    let browsers = find_registered_browsers();

    // Create application
    let app = gtk4::Application::new(Some("com.redirector.app"), Default::default());
    let window = gtk4::ApplicationWindow::new(&app);
    window.set_title(Some("Redirector"));
    window.set_default_size(650, 700);
    window.set_resizable(true);

    // Toast overlay
    let toast_overlay = libadwaita::ToastOverlay::new();

    // Header bar
    let header_bar = libadwaita::HeaderBar::new();

    // Close button
    let close_btn = Button::new();
    close_btn.set_icon_name("window-close-symbolic");
    close_btn.add_css_class("flat");
    close_btn.connect_clicked(clone! {
        #[weak]
        window,
        move |_| {
            window.close();
        }
    });
    header_bar.pack_end(&close_btn);

    // Content area with proper margins
    let content = BoxContainer::new(Orientation::Vertical, 12);
    content.set_margin_start(24);
    content.set_margin_end(24);
    content.set_margin_top(16);
    content.set_margin_bottom(16);

    // Title
    let title = Label::new(Some("URL Processed"));
    title.add_css_class("heading");
    title.set_halign(Align::Center);

    // Original URL card
    let orig_card = BoxContainer::new(Orientation::Vertical, 8);
    orig_card.add_css_class("card");
    orig_card.set_margin_top(8);

    let orig_label = Label::new(Some("Original:"));
    orig_label.add_css_class("dim-label");
    orig_label.set_halign(Align::Start);
    orig_label.set_margin_start(12);
    orig_label.set_margin_top(8);

    let orig_url = Label::new(Some(original));
    orig_url.set_wrap(true);
    orig_url.set_max_width_chars(60);
    orig_url.set_selectable(true);
    orig_url.set_halign(Align::Start);
    orig_url.set_hexpand(true);
    orig_url.set_margin_start(12);
    orig_url.set_margin_end(12);
    orig_url.set_margin_bottom(8);
    orig_url.set_css_classes(&["monospace"]); // monospace is a libadwaita CSS class

    orig_card.append(&orig_label);
    orig_card.append(&orig_url);

    // Arrow
    let arrow = Label::new(Some("↓"));
    arrow.set_halign(Align::Center);
    arrow.set_valign(Align::Center);
    arrow.add_css_class("dim-label");

    // Result URL card
    let result_card = BoxContainer::new(Orientation::Vertical, 8);
    result_card.add_css_class("card");

    let result_label = Label::new(Some("Result:"));
    result_label.add_css_class("dim-label");
    result_label.set_halign(Align::Start);
    result_label.set_margin_start(12);
    result_label.set_margin_top(8);

    let result_url_label = Label::new(Some(&result.url));
    result_url_label.set_wrap(true);
    result_url_label.set_max_width_chars(60);
    result_url_label.set_selectable(true);
    result_url_label.set_halign(Align::Start);
    result_url_label.set_hexpand(true);
    result_url_label.set_margin_start(12);
    result_url_label.set_margin_end(12);
    result_url_label.set_margin_bottom(8);
    result_url_label.set_css_classes(&["monospace"]);

    result_card.append(&result_label);
    result_card.append(&result_url_label);

    content.append(&title);
    content.append(&orig_card);
    content.append(&arrow);
    content.append(&result_card);

    // Changes section
    if !result.changes.is_empty() {
        let changes_sep = gtk4::Separator::new(Orientation::Horizontal);
        content.append(&changes_sep);

        let changes_group = libadwaita::PreferencesGroup::new();
        changes_group.set_title("Changes");

        for change in &result.changes {
            let card = BoxContainer::new(Orientation::Vertical, 6);
            card.add_css_class("card");
            card.set_margin_start(4);
            card.set_margin_end(4);
            card.set_margin_top(8);
            card.set_margin_bottom(8);

            // Module name badge
            let module_label = Label::new(Some(change.module));
            module_label.add_css_class("heading");
            module_label.set_halign(Align::Start);
            module_label.set_margin_start(12);
            module_label.set_margin_top(8);

            // Before
            let before_box = BoxContainer::new(Orientation::Horizontal, 8);
            before_box.set_hexpand(true);
            before_box.set_margin_start(12);
            before_box.set_margin_end(12);

            let before_label = Label::new(Some("Before:"));
            before_label.add_css_class("dim-label");
            before_label.set_halign(Align::Start);
            before_label.set_valign(Align::Start);

            let before_url = Label::new(Some(&change.original));
            before_url.set_wrap(true);
            before_url.set_max_width_chars(55);
            before_url.set_selectable(true);
            before_url.set_halign(Align::Start);
            before_url.set_hexpand(true);
            before_url.set_css_classes(&["monospace", "dim-label"]);

            before_box.append(&before_label);
            before_box.append(&before_url);

            // After
            let after_box = BoxContainer::new(Orientation::Horizontal, 8);
            after_box.set_hexpand(true);
            after_box.set_margin_start(12);
            after_box.set_margin_end(12);
            after_box.set_margin_top(4);

            let after_label = Label::new(Some("After:"));
            after_label.add_css_class("dim-label");
            after_label.set_halign(Align::Start);
            after_label.set_valign(Align::Start);

            let after_url = Label::new(Some(&change.result));
            after_url.set_wrap(true);
            after_url.set_max_width_chars(55);
            after_url.set_selectable(true);
            after_url.set_halign(Align::Start);
            after_url.set_hexpand(true);
            after_url.set_css_classes(&["monospace"]);

            after_box.append(&after_label);
            after_box.append(&after_url);

            card.append(&module_label);
            card.append(&before_box);
            card.append(&after_box);

            // Bottom padding inside card
            let pad = gtk4::Box::new(Orientation::Vertical, 0);
            pad.set_hexpand(false);
            pad.set_size_request(-1, 12);
            card.append(&pad);

            changes_group.add(&card);
        }

        let scrolled = gtk4::ScrolledWindow::new();
        scrolled.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Automatic);
        scrolled.set_vexpand(true);
        scrolled.set_child(Some(&changes_group));
        content.append(&scrolled);
    }

    // Browser selector
    let browser_row_box = BoxContainer::new(Orientation::Horizontal, 8);
    browser_row_box.set_hexpand(true);
    browser_row_box.set_halign(Align::Start);
    browser_row_box.set_margin_top(8);

    let browser_label = Label::new(Some("Open in:"));
    browser_label.add_css_class("dim-label");

    let store = gtk4::StringList::new(&browsers.iter().map(|b| b.name.as_str()).collect::<Vec<_>>());
    let selection = gtk4::SingleSelection::new(Some(store.clone()));
    let browser_combo = gtk4::DropDown::new(Some(selection), None::<&gtk4::Expression>);
    browser_combo.set_hexpand(true);

    browser_row_box.append(&browser_label);
    browser_row_box.append(&browser_combo);
    content.append(&browser_row_box);

    // Button row
    let btn_box = BoxContainer::new(Orientation::Horizontal, 8);
    btn_box.set_hexpand(true);
    btn_box.set_halign(Align::End);
    btn_box.set_margin_top(12);

    let open_btn = Button::new();
    open_btn.set_label("Open");
    open_btn.add_css_class("suggested-action");
    let result_url = result.url.clone();
    let browsers = browsers.clone();
    open_btn.connect_clicked(clone! {
        #[weak]
        window,
        move |_| {
            let sel = browser_combo.selected() as usize;
            if sel < browsers.len() {
                let browser = &browsers[sel];
                let _ = open_via_gio(&browser.desktop_id, &result_url);
            } else {
                let _ = open_url_default(&result_url);
            }
            window.close();
        }
    });

    let copy_btn = Button::new();
    copy_btn.set_label("Copy URL");
    let result_url2 = result.url.clone();
    copy_btn.connect_clicked(clone! {
        #[weak]
        toast_overlay,
        move |_| {
            let display = gtk4::gdk::Display::default().expect("Could not open display");
            let clipboard = display.clipboard();
            clipboard.set_text(&result_url2);
            let toast = libadwaita::Toast::new("URL copied to clipboard");
            toast.set_timeout(2);
            toast_overlay.add_toast(toast);
        }
    });

    btn_box.append(&open_btn);
    btn_box.append(&copy_btn);
    content.append(&btn_box);

    // Set header bar as titlebar
    window.set_titlebar(Some(&header_bar));

    // Set content on toast overlay
    toast_overlay.set_child(Some(&content));
    window.set_child(Some(&toast_overlay));

    window.present();

    // Quit the main loop when the window is closed
    let main_loop = gtk4::glib::MainLoop::new(None, false);
    window.connect_close_request(clone! {
        #[strong]
        main_loop,
        move |_| {
            main_loop.quit();
            glib::Propagation::Proceed
        }
    });
    main_loop.run();
}

/// Open a URL using `gtk-launch` with a specific desktop ID.
fn open_via_gio(desktop_id: &str, url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut child = std::process::Command::new("gtk-launch")
        .args([desktop_id, url])
        .spawn()?;
    child.wait()?;
    Ok(())
}

/// Open a URL in the system's default browser using xdg-open.
fn open_url_default(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut child = std::process::Command::new("xdg-open")
        .arg(url)
        .spawn()?;
    child.wait()?;
    Ok(())
}


