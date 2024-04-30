use glib::clone;
use gtk::prelude::*;
use hyprland::{
    dispatch::{Dispatch, DispatchType, WorkspaceIdentifierWithSpecial},
    shared::{Address, HyprData, HyprDataActive},
};
use std::collections::HashMap;
use tokio::{
    runtime::Runtime,
    sync::mpsc::{self, Sender},
};

enum Msg {
    AddWorkspace(i32),
    DeleteWorkspace(i32),
    ChangeActiveWorkspace(i32),
    ChangeActiveWindow(Address),
    OpenWindow(Address, String, i32),
    CloseWindow(Address),
    MoveWindow(Address, i32),
}

struct WorkspacesData {
    active_id: i32,
    workspace_btns: HashMap<i32, gtk::Button>,
    clients: HashMap<Address, (String, i32)>,
    default_icon: gtk::IconPaintable,
}

impl WorkspacesData {
    fn new(main_box: &gtk::Box, icon_theme: &gtk::IconTheme) -> Self {
        let clients: HashMap<_, _> = hyprland::data::Clients::get()
            .expect("Get clients data.")
            .map(|x| (x.address, (x.class, x.workspace.id)))
            .collect();

        let active_id = hyprland::data::Workspace::get_active()
            .expect("Get active workspace.")
            .id;

        let default_icon = icon_theme.lookup_icon(
            "display",
            &[],
            48,
            2,
            gtk::TextDirection::Ltr,
            gtk::IconLookupFlags::empty(),
        );

        let workspace_btns = create_workspace_btns(10, main_box, &default_icon);

        Self {
            active_id,
            workspace_btns,
            clients,
            default_icon,
        }
    }
}

pub fn get_module(icon_theme: &gtk::IconTheme, rt: &Runtime) -> gtk::Box {
    let bx = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Start)
        .build();

    let (tx, mut rx) = mpsc::channel(10);

    rt.spawn(async move {
        create_workspace_event_listener(tx)
            .start_listener_async()
            .await
            .expect("Hyprland event listener starts.");
    });

    let mut data = WorkspacesData::new(&bx, icon_theme);
    glib::spawn_future_local(clone!(@weak icon_theme => async move {
        refresh_icons(&data, &icon_theme);
        setup_active_workspace_btn(&data);

        while let Some(resp) = rx.recv().await {
            handle_response(resp, &mut data, &icon_theme);
        }
    }));

    bx
}

fn create_workspace_btns(
    capacity: usize,
    main_box: &gtk::Box,
    default_icon: &gtk::IconPaintable,
) -> HashMap<i32, gtk::Button> {
    let mut workspace_btns: HashMap<i32, gtk::Button> = HashMap::with_capacity(capacity);
    for id in 1..11 {
        let icon = gtk::Image::new();
        let workspace_btn = gtk::Button::builder()
            .visible(false)
            .halign(gtk::Align::Start)
            .child(&icon)
            .build();
        workspace_btn.add_css_class("workspace_button");
        icon.set_from_paintable(Some(default_icon));

        workspace_btn.connect_clicked(move |_| {
            hyprland::dispatch!(Workspace, WorkspaceIdentifierWithSpecial::Id(id))
                .expect("Failed to dispatch workspace command");
        });

        main_box.append(&workspace_btn);
        workspace_btns.insert(id, workspace_btn);
    }

    workspace_btns
}

fn create_workspace_event_listener(tx: Sender<Msg>) -> hyprland::event_listener::EventListener {
    let mut event_listener = hyprland::event_listener::EventListener::new();

    let d_tx = tx.clone();
    event_listener.add_workspace_added_handler(move |data| {
        if let hyprland::shared::WorkspaceType::Regular(id) = data {
            let id = id.parse().expect("Workspace id should be string number.");

            futures::executor::block_on(d_tx.send(Msg::AddWorkspace(id))).unwrap();
        }
    });

    let d_tx = tx.clone();
    event_listener.add_workspace_change_handler(move |data| {
        if let hyprland::shared::WorkspaceType::Regular(id) = data {
            let id = id.parse().expect("Workspace id should be string number.");

            futures::executor::block_on(d_tx.send(Msg::ChangeActiveWorkspace(id))).unwrap();
        }
    });

    let d_tx = tx.clone();
    event_listener.add_workspace_destroy_handler(move |data| {
        if let hyprland::shared::WorkspaceType::Regular(id) = data {
            let id = id.parse().expect("Workspace id should be string number.");

            futures::executor::block_on(d_tx.send(Msg::DeleteWorkspace(id))).unwrap();
        }
    });

    let d_tx = tx.clone();
    event_listener.add_active_window_change_handler(move |data| {
        if let Some(data) = data {
            futures::executor::block_on(d_tx.send(Msg::ChangeActiveWindow(data.window_address)))
                .unwrap();
        }
    });

    let d_tx = tx.clone();
    event_listener.add_window_open_handler(move |data| {
        let id = data
            .workspace_name
            .parse::<i32>()
            .expect("Workspace id should be an integer");
        futures::executor::block_on(d_tx.send(Msg::OpenWindow(
            data.window_address,
            data.window_class,
            id,
        )))
        .unwrap();
    });

    let d_tx = tx.clone();
    event_listener.add_window_close_handler(move |address| {
        futures::executor::block_on(d_tx.send(Msg::CloseWindow(address))).unwrap();
    });

    event_listener.add_window_moved_handler(move |data| {
        let id = data
            .workspace_name
            .parse::<i32>()
            .expect("Workspace id should be an integer");
        futures::executor::block_on(tx.send(Msg::MoveWindow(data.window_address, id))).unwrap();
    });

    event_listener
}

fn handle_response(resp: Msg, data: &mut WorkspacesData, icon_theme: &gtk::IconTheme) {
    match resp {
        Msg::AddWorkspace(id) => {
            data.workspace_btns
                .get(&id)
                .expect("Can't find workspace")
                .set_visible(true);
        }
        Msg::ChangeActiveWorkspace(id) => {
            change_workspace_focus(id, &mut data.active_id, &data.workspace_btns);
        }
        Msg::DeleteWorkspace(id) => {
            data.workspace_btns
                .get(&id)
                .expect("Can't find workspace")
                .set_visible(false);
        }
        Msg::ChangeActiveWindow(address) => {
            if let Some((name, id)) = data.clients.get(&address) {
                set_button_icon(id, name, data, icon_theme);
            }
        }
        Msg::OpenWindow(address, class, id) => {
            data.clients.insert(address, (class, id));
        }
        Msg::CloseWindow(address) => {
            data.clients.remove(&address);
            clean_empty_workspaces(data);
        }
        Msg::MoveWindow(address, id) => {
            if let Some((_, old_id)) = data.clients.get_mut(&address) {
                *old_id = id;
            }
            clean_empty_workspaces(data);
        }
    }
}

fn set_button_icon(id: &i32, class: &str, data: &WorkspacesData, icon_theme: &gtk::IconTheme) {
    let btn = data
        .workspace_btns
        .get(id)
        .expect("Workspace button not found");
    let img = btn
        .child()
        .and_downcast::<gtk::Image>()
        .expect("Workspace button should be image.");
    let icon = icon_theme.lookup_icon(
        class,
        &[class.to_lowercase().as_str(), "display"],
        48,
        2,
        gtk::TextDirection::Ltr,
        gtk::IconLookupFlags::empty(),
    );
    img.set_paintable(Some(&icon));
}

fn change_workspace_focus(
    id: i32,
    active_id: &mut i32,
    workspace_btns: &HashMap<i32, gtk::Button>,
) {
    let btn = workspace_btns
        .get(active_id)
        .expect("Old active workspace not found.");
    btn.set_sensitive(true);

    let btn = workspace_btns
        .get(&id)
        .expect("New active workspace not found.");
    btn.set_sensitive(false);

    *active_id = id;
}

fn setup_active_workspace_btn(data: &WorkspacesData) {
    let btn = data
        .workspace_btns
        .get(&data.active_id)
        .expect("Can't find active workspace");
    btn.set_visible(true);
    btn.set_sensitive(false);
}

fn refresh_icons(data: &WorkspacesData, icon_theme: &gtk::IconTheme) {
    for (id, btn) in data.workspace_btns.iter() {
        for (class, id_btn) in data.clients.values() {
            if id == id_btn {
                set_button_icon(id, class, data, icon_theme);
                btn.set_visible(true);
                if *id == data.active_id {
                    btn.set_sensitive(false);
                }
                break;
            }
        }
    }
}

fn clean_empty_workspaces(data: &WorkspacesData) {
    data.workspace_btns
        .iter()
        .filter(|x| data.clients.values().filter(|el| el.1 == *x.0).count() == 0)
        .map(|x| {
            x.1.child()
                .and_downcast::<gtk::Image>()
                .expect("Workspaces btn child should be image")
        })
        .for_each(|img| {
            img.set_from_paintable(Some(&data.default_icon));
        });
}
