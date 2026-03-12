use gtk4::prelude::*;
use gtk4::{Box, Image, Label, Orientation};
use std::cell::RefCell;
use widget_poll::Poller;

thread_local! {
    static CPU_LABEL: RefCell<Option<Label>> = RefCell::new(None);
    static MEM_LABEL: RefCell<Option<Label>> = RefCell::new(None);
    static BAT_LABEL: RefCell<Option<Label>> = RefCell::new(None);
    static POLLER: RefCell<Option<Poller>> = RefCell::new(None);
}

pub fn build() -> Box {
    let container = Box::new(Orientation::Horizontal, 8);
    container.add_css_class("stats-section");
    container.set_homogeneous(true);

    // Initialize poller
    POLLER.with(|p| *p.borrow_mut() = Some(Poller::new()));

    // CPU
    let cpu_box = create_stat_item("cpu-symbolic", "CPU", "--");
    container.append(&cpu_box.0);
    CPU_LABEL.with(|l| *l.borrow_mut() = Some(cpu_box.1));

    // Memory
    let mem_box = create_stat_item("drive-harddisk-symbolic", "RAM", "--");
    container.append(&mem_box.0);
    MEM_LABEL.with(|l| *l.borrow_mut() = Some(mem_box.1));

    // Battery
    let bat_box = create_stat_item("battery-symbolic", "BAT", "--");
    container.append(&bat_box.0);
    BAT_LABEL.with(|l| *l.borrow_mut() = Some(bat_box.1));

    // Initial update
    update();

    container
}

fn create_stat_item(icon_name: &str, _label: &str, value: &str) -> (Box, Label) {
    let item = Box::new(Orientation::Horizontal, 4);
    item.add_css_class("stat-item");
    item.set_halign(gtk4::Align::Center);

    let icon = Image::from_icon_name(icon_name);
    icon.add_css_class("stat-icon");
    item.append(&icon);

    let value_label = Label::new(Some(value));
    value_label.add_css_class("stat-value");
    item.append(&value_label);

    (item, value_label)
}

pub fn update() {
    POLLER.with(|p| {
        if let Some(poller) = p.borrow().as_ref() {
            // CPU
            let cpu = poller.cpu();
            CPU_LABEL.with(|l| {
                if let Some(label) = l.borrow().as_ref() {
                    label.set_text(&format!("{:.0}%", cpu.usage));
                }
            });

            // Memory
            let mem = poller.memory();
            MEM_LABEL.with(|l| {
                if let Some(label) = l.borrow().as_ref() {
                    label.set_text(&format!("{:.0}%", mem.usage));
                }
            });

            // Battery
            if let Some(bat) = poller.battery() {
                BAT_LABEL.with(|l| {
                    if let Some(label) = l.borrow().as_ref() {
                        label.set_text(&format!("{}%", bat.percentage));
                    }
                });
            }
        }
    });
}
