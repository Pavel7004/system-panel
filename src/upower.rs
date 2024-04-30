use glib::clone;
use gtk::prelude::*;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use zbus::export::futures_util::StreamExt;
use zbus::{dbus_proxy, Connection, Result};

#[dbus_proxy(
    interface = "org.freedesktop.UPower.Device",
    default_service = "org.freedesktop.UPower",
    default_path = "/org/freedesktop/UPower/devices/battery_BAT0"
)]
trait UPower {
    #[dbus_proxy(property)]
    fn icon_name(&self) -> Result<String>;
}

pub fn get_module(icon_theme: &gtk::IconTheme, rt: &Runtime) -> gtk::Box {
    let (tx, mut rx) = mpsc::channel(1);

    rt.spawn(async move {
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
    glib::spawn_future_local(clone!(@weak icon, @weak icon_theme => async move {
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
