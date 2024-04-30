use chrono::Local;
use glib::clone;
use glib::once_cell::sync::Lazy;
use gtk::prelude::*;
use gtk::{glib, Application, ApplicationWindow};
use gtk_layer_shell::{Edge, Layer, LayerShell};
use tokio::runtime::Runtime;

mod hyprland;
mod upower;

const APP_ID: &str = "com.github.Pavel7004.system-panel-rs";

static RUNTIME: Lazy<Runtime> =
    Lazy::new(|| Runtime::new().expect("Setting up tokio runtime needs to succeed."));

fn main() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();

    app.connect_activate(build_ui);

    app.run()
}

fn build_ui(app: &Application) {
    let display = gdk::Display::default().expect("Can't connect to default display");
    let icon_theme = gtk::IconTheme::for_display(&display); // TODO: maybe handle icon theme change
    load_styles(&display);

    let window = ApplicationWindow::builder()
        .display(&display)
        .application(app)
        .title("My GTK App")
        .child(&build_main_box(&icon_theme))
        .build();

    window.init_layer_shell();
    window.set_layer(Layer::Top);
    window.auto_exclusive_zone_enable();
    window.set_anchor(Edge::Top, true);
    window.set_anchor(Edge::Bottom, true);
    window.set_anchor(Edge::Left, true);

    window.present();
}

fn load_styles(display: &gdk::Display) {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(include_str!("style.css"));
    // TODO: Load style file in runtime
    //provider.load_from_path("style.css");

    gtk::style_context_add_provider_for_display(
        display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_USER,
    );
}

fn build_main_box(icon_theme: &gtk::IconTheme) -> gtk::Box {
    let main_box = gtk::Box::builder()
        .hexpand(true)
        .vexpand(true)
        .halign(gtk::Align::Start)
        .orientation(gtk::Orientation::Vertical)
        .name("main_box")
        .homogeneous(true)
        .build();

    main_box.append(&build_left_side(icon_theme));
    main_box.append(&build_middle_side(icon_theme));
    main_box.append(&build_right_side(icon_theme));

    main_box
}

fn build_left_side(icon_theme: &gtk::IconTheme) -> gtk::Box {
    let bx = gtk::Box::builder()
        .name("left_box")
        .orientation(gtk::Orientation::Vertical)
        .valign(gtk::Align::Start)
        .build();

    bx.append(&hyprland::get_module(icon_theme, &RUNTIME));

    bx
}

fn build_middle_side(_icon_theme: &gtk::IconTheme) -> gtk::Box {
    let bx = gtk::Box::builder()
        .name("middle_box")
        .orientation(gtk::Orientation::Vertical)
        .valign(gtk::Align::Center)
        .build();

    bx.append(&get_clock_module());

    bx
}

fn build_right_side(icon_theme: &gtk::IconTheme) -> gtk::Box {
    let bx = gtk::Box::builder()
        .name("right_box")
        .orientation(gtk::Orientation::Vertical)
        .valign(gtk::Align::End)
        .build();

    bx.append(&build_misc_area(icon_theme));

    bx
}

fn get_clock_module() -> gtk::Button {
    let hour_label = gtk::Label::builder().label("99").build();
    let dots_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .halign(gtk::Align::Fill)
        .build();
    dots_box.add_css_class("separator");
    for _ in 0..2 {
        let tmp_box = gtk::Box::builder()
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .hexpand(true)
            .build();
        tmp_box.add_css_class("dot");

        dots_box.append(&tmp_box);
    }
    let minutes_label = gtk::Label::builder().label("99").build();

    let time_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .vexpand(true)
        .valign(gtk::Align::Center)
        .build();

    time_box.append(&hour_label);
    time_box.append(&dots_box);
    time_box.append(&minutes_label);

    glib::timeout_add_seconds_local(
        1,
        clone!(@weak hour_label, @weak minutes_label => @default-return glib::ControlFlow::Break, move || {
            let now = Local::now().format("%H:%M").to_string();

            let data: Vec<&str> = now.split(':').collect();
            if data.len() < 2 {
                return glib::ControlFlow::Break
            }

            let hour = data[0];
            let minutes = data[1];

            hour_label.set_label(hour);
            minutes_label.set_label(minutes);

            glib::ControlFlow::Continue
        }),
    );

    gtk::Button::builder().child(&time_box).build()
}

fn build_misc_area(icon_theme: &gtk::IconTheme) -> gtk::Button {
    let btn_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .valign(gtk::Align::Center)
        .halign(gtk::Align::Center)
        .homogeneous(true)
        .build();

    let btn = gtk::Button::builder().child(&btn_box).build();

    btn_box.append(&upower::get_module(icon_theme, &RUNTIME));

    btn
}
