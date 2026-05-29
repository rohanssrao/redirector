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
        gdk, glib, prelude::*, Align, Button, Label, ListBox, ListBoxRow,
        Orientation, PolicyType, ScrolledWindow, SelectionMode,
    };
    use glib::clone;
    use libadwaita::{self, prelude::*};

    gtk4::init().unwrap();
    libadwaita::init().unwrap();

    let original = original.to_string();
    let result_url = result.url.clone();
    let changes = result.changes.clone();
    let browsers = find_registered_browsers();

    let app = gtk4::Application::builder()
        .application_id("com.redirector.app")
        .build();

    let window = libadwaita::ApplicationWindow::builder()
        .application(&app)
        .title("Redirector")
        .default_width(650)
        .default_height(700)
        .build();

    let toast_overlay = libadwaita::ToastOverlay::new();
    let toolbar_view = libadwaita::ToolbarView::new();
    toolbar_view.add_top_bar(&libadwaita::HeaderBar::new());

    let clamp = libadwaita::Clamp::builder()
        .maximum_size(720)
        .tightening_threshold(560)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .build();

    let content = gtk4::Box::new(Orientation::Vertical, 18);
    clamp.set_child(Some(&content));

    // Title
    let title = Label::new(Some("URL Processed"));
    title.add_css_class("title-1");
    title.set_halign(Align::Center);
    content.append(&title);

    // Original URL card
    let orig_card = libadwaita::Bin::new();
    orig_card.add_css_class("card");
    let orig_vbox = gtk4::Box::new(Orientation::Vertical, 6);
    orig_vbox.set_margin_top(12);
    orig_vbox.set_margin_bottom(12);
    orig_vbox.set_margin_start(12);
    orig_vbox.set_margin_end(12);

    let orig_cap = Label::new(Some("Original"));
    orig_cap.add_css_class("caption-heading");
    orig_cap.add_css_class("dim-label");
    orig_cap.set_halign(Align::Start);

    let orig_lbl = Label::new(Some(&original));
    orig_lbl.add_css_class("monospace");
    orig_lbl.set_wrap(true);
    orig_lbl.set_selectable(true);
    orig_lbl.set_halign(Align::Fill);
    orig_lbl.set_xalign(0.0);

    orig_vbox.append(&orig_cap);
    orig_vbox.append(&orig_lbl);
    orig_card.set_child(Some(&orig_vbox));
    content.append(&orig_card);

    // Arrow
    let arrow = Label::new(Some("↓"));
    arrow.add_css_class("dim-label");
    arrow.set_halign(Align::Center);
    content.append(&arrow);

    // Result URL card
    let res_card = libadwaita::Bin::new();
    res_card.add_css_class("card");
    let res_vbox = gtk4::Box::new(Orientation::Vertical, 6);
    res_vbox.set_margin_top(12);
    res_vbox.set_margin_bottom(12);
    res_vbox.set_margin_start(12);
    res_vbox.set_margin_end(12);

    let res_cap = Label::new(Some("Result"));
    res_cap.add_css_class("caption-heading");
    res_cap.add_css_class("dim-label");
    res_cap.set_halign(Align::Start);

    let res_lbl = Label::new(Some(&result_url));
    res_lbl.add_css_class("monospace");
    res_lbl.set_wrap(true);
    res_lbl.set_selectable(true);
    res_lbl.set_halign(Align::Fill);
    res_lbl.set_xalign(0.0);

    res_vbox.append(&res_cap);
    res_vbox.append(&res_lbl);
    res_card.set_child(Some(&res_vbox));
    content.append(&res_card);

    // Changes log
    if !changes.is_empty() {
        let changes_title = Label::new(Some("Changes"));
        changes_title.add_css_class("heading");
        changes_title.set_halign(Align::Start);
        changes_title.set_margin_top(6);
        content.append(&changes_title);

        let list = ListBox::new();
        list.add_css_class("boxed-list");
        list.set_selection_mode(SelectionMode::None);

        for change in &changes {
            let row = ListBoxRow::new();
            row.set_activatable(false);

            let row_box = gtk4::Box::new(Orientation::Vertical, 12);
            row_box.set_margin_top(12);
            row_box.set_margin_bottom(12);
            row_box.set_margin_start(12);
            row_box.set_margin_end(12);

            let module = Label::new(Some(&change.module));
            module.add_css_class("heading");
            module.set_halign(Align::Start);

            let before_box = gtk4::Box::new(Orientation::Vertical, 4);
            let before_cap = Label::new(Some("Before"));
            before_cap.add_css_class("caption");
            before_cap.add_css_class("dim-label");
            before_cap.set_halign(Align::Start);

            let before_val = Label::new(Some(&change.original));
            before_val.add_css_class("monospace");
            before_val.add_css_class("dim-label");
            before_val.set_wrap(true);
            before_val.set_selectable(true);
            before_val.set_halign(Align::Fill);
            before_val.set_xalign(0.0);

            before_box.append(&before_cap);
            before_box.append(&before_val);

            let after_box = gtk4::Box::new(Orientation::Vertical, 4);
            let after_cap = Label::new(Some("After"));
            after_cap.add_css_class("caption");
            after_cap.add_css_class("dim-label");
            after_cap.set_halign(Align::Start);

            let after_val = Label::new(Some(&change.result));
            after_val.add_css_class("monospace");
            after_val.set_wrap(true);
            after_val.set_selectable(true);
            after_val.set_halign(Align::Fill);
            after_val.set_xalign(0.0);

            after_box.append(&after_cap);
            after_box.append(&after_val);

            row_box.append(&module);
            row_box.append(&before_box);
            row_box.append(&after_box);

            row.set_child(Some(&row_box));
            list.append(&row);
        }

        let scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .vexpand(true)
            .child(&list)
            .build();

        content.append(&scrolled);
    }

    // Browser selector
    let browser_group = libadwaita::PreferencesGroup::new();
    browser_group.set_title("Destination");

    let browser_names: Vec<String> = browsers.iter().map(|b| b.name.clone()).collect();
    let string_list = gtk4::StringList::new(
        &browser_names.iter().map(|s| s.as_str()).collect::<Vec<_>>()
    );

    let combo_row = libadwaita::ComboRow::new();
    combo_row.set_title("Browser");
    combo_row.set_model(Some(&string_list));
    if !browser_names.is_empty() {
        combo_row.set_selected(0);
    }

    browser_group.add(&combo_row);
    content.append(&browser_group);

    // Actions
    let btn_box = gtk4::Box::new(Orientation::Horizontal, 8);
    btn_box.set_halign(Align::End);
    btn_box.set_margin_top(12);

    let copy_btn = Button::with_label("Copy URL");
    let open_btn = Button::with_label("Open");
    open_btn.add_css_class("suggested-action");

    let browsers_for_open = browsers.clone();
    let url_for_open = result_url.clone();
    open_btn.connect_clicked(clone! {
        #[weak]
        window,
        #[weak]
        combo_row,
        move |_| {
            let idx = combo_row.selected() as usize;
            let url = url_for_open.clone();
            if idx < browsers_for_open.len() {
                let _ = open_via_gio(&browsers_for_open[idx].desktop_id, &url);
            } else {
                let _ = open_url_default(&url);
            }
            window.close();
        }
    });

    let url_for_copy = result_url.clone();
    copy_btn.connect_clicked(clone! {
        #[weak]
        toast_overlay,
        move |_| {
            if let Some(display) = gdk::Display::default() {
                display.clipboard().set_text(&url_for_copy);
                let toast = libadwaita::Toast::new("URL copied to clipboard");
                toast.set_timeout(2);
                toast_overlay.add_toast(toast);
            }
        }
    });

    btn_box.append(&copy_btn);
    btn_box.append(&open_btn);
    content.append(&btn_box);

    toolbar_view.set_content(Some(&clamp));
    toast_overlay.set_child(Some(&toolbar_view));
    window.set_content(Some(&toast_overlay));

    let main_loop = glib::MainLoop::new(None, false);
    window.connect_close_request(clone! {
        #[strong]
        main_loop,
        move |_| {
            main_loop.quit();
            glib::Propagation::Proceed
        }
    });

    window.present();
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


