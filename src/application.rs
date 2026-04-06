// Copyright 2021 Tom A. Wagner <tom.a.wagner@protonmail.com>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License version 3 as published by
// the Free Software Foundation.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: GPL-3.0-only

use adw::{
    gio,
    glib::{self, clone},
    gtk,
    prelude::*,
    subclass::prelude::*,
};
use log::error;
use pipewire::channel::Sender;

use crate::{graph_manager::GraphManager, ui, GtkMessage, PipewireMessage};

static STYLE: &str = include_str!("style.css");
static APP_ID: &str = "org.pipewire.Helvum";
static VERSION: &str = env!("CARGO_PKG_VERSION");
static AUTHORS: &str = env!("CARGO_PKG_AUTHORS");

const DEFAULT_REMOTE_NAME: &str = "Default Remote";

mod imp {
    use super::*;

    use std::cell::OnceCell;

    use adw::subclass::prelude::AdwApplicationImpl;

    #[derive(Default)]
    pub struct Application {
        pub(super) window: ui::Window,
        pub(super) graph_manager: OnceCell<GraphManager>,
        pub(super) pw_sender: OnceCell<Sender<GtkMessage>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Application {
        const NAME: &'static str = "HelvumApplication";
        type Type = super::Application;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for Application {}
    impl ApplicationImpl for Application {
        fn activate(&self) {
            let app = &*self.obj();

            let graphview = self.window.graph();

            self.window.set_application(Some(app));

            let zoom_set_action =
                gio::SimpleAction::new("set-zoom", Some(&f64::static_variant_type()));
            zoom_set_action.connect_activate(clone!(@weak graphview => move|_, param| {
                let zoom_factor = param.unwrap().get::<f64>().unwrap();
                graphview.set_zoom_factor(zoom_factor, None)
            }));
            self.window.add_action(&zoom_set_action);

            self.window.show();
        }

        fn startup(&self) {
            self.parent_startup();

            self.obj()
                .style_manager()
                .set_color_scheme(adw::ColorScheme::PreferDark);

            // Load CSS from the STYLE variable.
            let provider = gtk::CssProvider::new();
            provider.load_from_data(STYLE);
            gtk::style_context_add_provider_for_display(
                &gtk::gdk::Display::default().expect("Error initializing gtk css provider."),
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );

            self.setup_actions();
        }
    }
    impl GtkApplicationImpl for Application {}
    impl AdwApplicationImpl for Application {}

    impl Application {
        fn setup_actions(&self) {
            let obj = &*self.obj();

            // Add <Control-Q> shortcut for quitting the application.
            let quit = gtk::gio::SimpleAction::new("quit", None);
            quit.connect_activate(clone!(@weak obj => move |_, _| {
                obj.quit();
            }));
            obj.set_accels_for_action("app.quit", &["<Control>Q"]);
            obj.add_action(&quit);

            let unify_stereo_links = gio::SimpleAction::new_stateful(
                "unify-stereo-links",
                None,
                &false.to_variant(),
            );
            unify_stereo_links.connect_change_state(clone!(@weak obj => move |action, value| {
                let enabled = value
                    .expect("unify-stereo-links action needs a state")
                    .get::<bool>()
                    .expect("unify-stereo-links state should be a bool");

                action.set_state(&enabled.to_variant());
                obj.imp().set_unify_stereo_links(enabled);
            }));
            obj.add_action(&unify_stereo_links);

            let action_about = gio::ActionEntry::builder("about")
                .activate(|obj: &super::Application, _, _| {
                    obj.imp().show_about_dialog();
                })
                .build();
            obj.add_action_entries([action_about]);

            let unify_stereo = gio::SimpleAction::new_stateful(
                "unify-stereo-connections",
                None,
                &true.to_variant(),
            );
            if let Some(graph_manager) = self.graph_manager.get() {
                graph_manager.set_unify_stereo_connections(true);
            }
            unify_stereo.connect_change_state(clone!(@weak self as imp => move |action, value| {
                let Some(value) = value else {
                    return;
                };
                let Some(enabled) = value.get::<bool>() else {
                    return;
                };

                action.set_state(value);
                if let Some(graph_manager) = imp.graph_manager.get() {
                    graph_manager.set_unify_stereo_connections(enabled);
                }
            }));
            obj.add_action(&unify_stereo);
        }

        fn show_about_dialog(&self) {
            let obj = &*self.obj();
            let window = obj.active_window().unwrap();
            let authors: Vec<&str> = AUTHORS.split(':').collect();

            let about_window = adw::AboutWindow::builder()
                .transient_for(&window)
                .application_icon(APP_ID)
                .application_name("Helvum")
                .developer_name("Tom Wagner")
                .developers(authors)
                .version(VERSION)
                .website("https://gitlab.freedesktop.org/pipewire/helvum")
                .issue_url("https://gitlab.freedesktop.org/pipewire/helvum/-/issues")
                .license_type(gtk::License::Gpl30Only)
                .build();

            about_window.present();
        }

        fn set_unify_stereo_links(&self, enabled: bool) {
            self.pw_sender
                .get()
                .expect("pw_sender should be set")
                .send(GtkMessage::SetUnifyStereoLinks(enabled))
                .expect("Failed to send stereo link mode message");
        }
    }
}

glib::wrapper! {
    pub struct Application(ObjectSubclass<imp::Application>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl Application {
    /// Create the view.
    /// This will set up the entire user interface and prepare it for being run.
    pub(super) fn new(
        gtk_receiver: async_channel::Receiver<PipewireMessage>,
        pw_sender: Sender<GtkMessage>,
    ) -> Self {
        let app: Application = glib::Object::builder()
            .property("application-id", APP_ID)
            .build();

        let imp = app.imp();

        imp.setup_options(pw_sender.clone());

        imp.graph_manager
            .set(GraphManager::new(
                &imp.window.graph(),
                &imp.window.connection_banner(),
                pw_sender.clone(),
                gtk_receiver,
            ))
            .expect("Should be able to set graph manager");

        assert!(
            imp.pw_sender.set(pw_sender).is_ok(),
            "Should be able to set pw sender"
        );

        app
    }
}
