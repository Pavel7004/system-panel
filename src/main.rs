use chrono::Local;
use glib::clone;
use glib::once_cell::sync::Lazy;
use gtk::prelude::*;
use gtk::{glib, Application, ApplicationWindow};
use gtk_layer_shell::{Edge, Layer, LayerShell};
use hyprland::dispatch::{Dispatch, DispatchType};
use hyprland::prelude::*;
use std::collections::HashMap;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use zbus::export::futures_util::StreamExt;
use zbus::{dbus_proxy, Connection, Result};

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

    main_box.append(&build_left_side(&icon_theme));
    main_box.append(&build_middle_side(&icon_theme));
    main_box.append(&build_right_side(&icon_theme));

    main_box
}

fn build_left_side(icon_theme: &gtk::IconTheme) -> gtk::Box {
    let bx = gtk::Box::builder()
        .name("left_box")
        .orientation(gtk::Orientation::Vertical)
        .valign(gtk::Align::Start)
        .build();

    bx.append(&get_workspaces_module(icon_theme));

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

            hour_label.set_label(&hour);
            minutes_label.set_label(&minutes);

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

    btn_box.append(&get_power_module(&icon_theme));

    btn
}

#[dbus_proxy(
    interface = "org.freedesktop.UPower.Device",
    default_service = "org.freedesktop.UPower",
    default_path = "/org/freedesktop/UPower/devices/battery_BAT0"
)]
trait UPower {
    #[dbus_proxy(property)]
    fn icon_name(&self) -> Result<String>;
}

fn get_power_module(icon_theme: &gtk::IconTheme) -> gtk::Box {
    let (tx, mut rx) = mpsc::channel(1);

    RUNTIME.spawn(async move {
        let conn = Connection::system()
            .await
            .expect("Can't connect to system dbus");

        let proxy = UPowerProxy::new(&conn)
            .await
            .expect("Can't create proxy for UPower");

        let mut stream = proxy.receive_icon_name_changed().await;
        while let Some(change) = stream.next().await {
            let icon_name = change.get().await.unwrap();

            tx.send(icon_name).await.unwrap();
        }
    });

    let icon = gtk::Image::new();
    // TODO: icon_theme needs to be weak reference
    glib::spawn_future_local(clone!(@weak icon, @strong icon_theme => async move {
        while let Some(icon_name) = rx.recv().await {
            let icon_paintable = icon_theme.lookup_icon(
                &icon_name,
                &[""],
                30,
                2,
                gtk::TextDirection::Ltr,
                gtk::IconLookupFlags::empty(),
            );

            icon.set_paintable(Some(&icon_paintable));
        }
    }));

    let power_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .halign(gtk::Align::Center)
        .spacing(1)
        .build();
    power_box.add_css_class("power");

    power_box.append(&icon);

    power_box
}

fn get_workspaces_module(icon_theme: &gtk::IconTheme) -> gtk::Box {
    let bx = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Start)
        .build();

    enum Msg {
        AddWorkspace(i32),
        DeleteWorkspace(i32),
        ChangeActiveWorkspace(i32),
        ChangeActiveWindow(i32, String),
    }

    let (tx, mut rx) = mpsc::channel(10);

    RUNTIME.spawn(async move {
        for workspace in hyprland::data::Workspaces::get_async()
            .await
            .expect("Can't get workspaces")
        {
            tx.send(Msg::AddWorkspace(workspace.id)).await.unwrap();

            tx.send(Msg::ChangeActiveWindow(
                workspace.id,
                workspace.last_window_title,
            ))
            .await
            .unwrap();
        }

        tx.send(Msg::ChangeActiveWorkspace(
            hyprland::data::Workspace::get_active_async()
                .await
                .expect("Can't get active workspace")
                .id,
        ))
        .await
        .unwrap();

        let mut event_listener = hyprland::event_listener::EventListener::new();

        let d_tx = tx.clone();
        event_listener.add_workspace_added_handler(move |data| {
            let tx = d_tx.clone();

            if let hyprland::shared::WorkspaceType::Regular(id) = data {
                RUNTIME.spawn(async move {
                    tx.send(Msg::AddWorkspace(
                        id.parse().expect("Workspace id should be string number."),
                    ))
                    .await
                    .unwrap();
                });
            };
        });

        let d_tx = tx.clone();
        event_listener.add_workspace_change_handler(move |data| {
            let tx = d_tx.clone();

            if let hyprland::shared::WorkspaceType::Regular(id) = data {
                RUNTIME.spawn(async move {
                    tx.send(Msg::ChangeActiveWorkspace(
                        id.parse().expect("Workspace id should be string number."),
                    ))
                    .await
                    .unwrap();
                });
            };
        });

        event_listener.add_workspace_destroy_handler(move |data| {
            let tx = tx.clone();

            if let hyprland::shared::WorkspaceType::Regular(id) = data {
                RUNTIME.spawn(async move {
                    tx.send(Msg::DeleteWorkspace(
                        id.parse().expect("Workspace id should be string number."),
                    ))
                    .await
                    .unwrap();
                });
            };
        });

        event_listener
            .start_listener_async()
            .await
            .expect("Hyprland event listener ended with error");
    });

    glib::spawn_future_local(clone!(@weak bx, @weak icon_theme => async move {
        let desktop = icon_theme.lookup_icon("display",
            &["gnome-dev-computer"],
            48,
            2,
            gtk::TextDirection::Ltr,
            gtk::IconLookupFlags::empty(),
        );

        let mut workspace_btns: HashMap<i32, gtk::Button> = HashMap::with_capacity(10);
        let mut active_id: i32 = 1;

        for id in 1..11 {
            let icon = gtk::Image::new();
            let workspace_btn = gtk::Button::builder()
                .visible(false)
                .halign(gtk::Align::Start)
                .child(&icon)
                .build();
            workspace_btn.add_css_class("workspace_button");
            icon.set_paintable(Some(&desktop));

            workspace_btn.connect_clicked(move |_| {
                let id = id.clone();
                hyprland::dispatch!(Workspace, hyprland::dispatch::WorkspaceIdentifierWithSpecial::Id(id)).expect("Failed to dispatch workspace command");
            });

            bx.append(&workspace_btn);
            workspace_btns.insert(id, workspace_btn);
        }

        while let Some(resp) = rx.recv().await {
            match resp {
                Msg::AddWorkspace(id) => {
                    workspace_btns.get(&id).expect("Can't find workspace").set_visible(true);
                },
                Msg::ChangeActiveWorkspace(id) => {
                    let btn = workspace_btns.get(&active_id).expect("Old active workspace not found.");
                    btn.set_sensitive(true);

                    let btn = workspace_btns.get(&id).expect("New active workspace not found.");
                    btn.set_sensitive(false);

                    active_id = id;
                },
                Msg::DeleteWorkspace(id) => {
                    workspace_btns.get(&id).expect("Can't find workspace").set_visible(false);
                },
                Msg::ChangeActiveWindow(_id, _name) => {},
            }
        }
    }));

    bx
}
