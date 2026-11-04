// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! The account: server or Google, the password (asked, remembered or from a command) and the sign-in.

use super::*;

impl qobject::CalendarModel {
    pub fn account(&self) -> QString {
        let state = self.sync_store().load();
        qs(json!({
            "provider": self.config.provider, "googleClientId": self.config.google_client_id,
            "googleClientSecret": self.config.google_client_secret,
            "server": self.config.server, "user": self.config.user, "davUrl": self.config.dav_url,
            "passwordCommand": self.config.password_command, "rememberPassword": self.config.remember_password,
            "lastSync": state.last_sync.map(describe_time), "error": state.error, "home": state.home,
        })
        .to_string())
    }

    pub fn test_connection(
        &self,
        server: &QString,
        user: &QString,
        password: &QString,
        dav_url: &QString,
        password_command: &QString,
    ) {
        let config = CalendarConfig {
            server: server.to_string(),
            user: user.to_string(),
            dav_url: dav_url.to_string(),
            ..Default::default()
        };
        // Empty: the password already given in this Atlas start, else the
        // command's.
        let password = Some(password.to_string()).filter(|p| !p.is_empty()).or_else(password_now);
        let command = password_command.to_string();
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let password = match password {
                Some(p) => Ok(p),
                None if !command.trim().is_empty() => calendar::password_from_command(&command, COMMAND_TIMEOUT)
                    .map_err(|e| anyhow::anyhow!("Passwort-Befehl: {e:#}")),
                None => Ok(String::new()),
            };
            let result = password.and_then(|p| client(&config, &p).discover().map_err(|e| anyhow::anyhow!("{e}")));
            let _ = thread.queue(move |this| match result {
                Ok(d) => {
                    let list: Vec<Value> = d
                        .calendars
                        .iter()
                        .map(|c| json!({ "url": c.url, "name": c.name, "color": c.color, "writable": c.writable }))
                        .collect();
                    let message = format!("Verbunden – {} Kalender gefunden", list.len());
                    this.connection_tested(true, qs(message), qs(Value::from(list).to_string()));
                }
                Err(err) => this.connection_tested(false, qs(err.to_string()), qs("[]")),
            });
        });
    }

    pub fn save_account(
        mut self: Pin<&mut Self>,
        server: &QString,
        user: &QString,
        password: &QString,
        dav_url: &QString,
        calendars: &QString,
        password_command: &QString,
    ) {
        let mut config = Config::load().calendar;
        let command = password_command.to_string().trim().to_owned();
        if config.password_command != command {
            reset_command();
        }
        config.password_command = command;
        let changed_account = config.server != server.to_string()
            || config.user != user.to_string()
            || config.dav_url != dav_url.to_string();
        config.server = server.to_string().trim().to_owned();
        config.user = user.to_string().trim().to_owned();
        config.dav_url = dav_url.to_string().trim().to_owned();
        if let Ok(Value::Array(list)) = serde_json::from_str::<Value>(&calendars.to_string()) {
            let old = std::mem::take(&mut config.calendars);
            for c in list {
                let Some(url) = c["url"].as_str() else { continue };
                let mut choice = old
                    .iter()
                    .find(|o| o.url == url)
                    .cloned()
                    .unwrap_or(CalendarChoice { url: url.to_owned(), ..Default::default() });
                choice.name = c["name"].as_str().unwrap_or_default().to_owned();
                choice.hidden = !c["visible"].as_bool().unwrap_or(true);
                config.calendars.push(choice);
            }
        }
        if !config.calendars.iter().any(|c| c.url == config.default) {
            config.default.clear();
        }
        if let Err(err) = Config::save_calendar(&config) {
            self.as_mut().set_error(format!("Einstellungen können nicht gespeichert werden: {err:#}"));
            return;
        }
        if changed_account {
            let _ = self.sync_store().clear();
            let _ = forget_secret();
        }
        let password = password.to_string();
        if !password.is_empty()
            && let Ok(mut p) = PASSWORD.lock()
        {
            *p = Some(password.clone());
        }
        if !password.is_empty() && config.remember_password {
            self.as_mut().keep_password(password);
        }
        self.as_mut().reload();
        self.sync_now();
    }

    pub fn remember_password(mut self: Pin<&mut Self>, remember: bool) {
        let mut config = Config::load().calendar;
        if config.remember_password != remember {
            config.remember_password = remember;
            if let Err(err) = Config::save_calendar(&config) {
                self.as_mut().set_error(format!("Einstellungen können nicht gespeichert werden: {err:#}"));
                return;
            }
            self.as_mut().rust_mut().config.remember_password = remember;
        }
        if !remember {
            if let Err(err) = forget_secret() {
                self.as_mut().set_error(format!("Gemerktes Passwort kann nicht gelöscht werden: {err:#}"));
            }
            self.as_mut().update_sync_state();
        } else if let Some(password) = password() {
            self.as_mut().keep_password(password);
        }
    }

    /// Stores the password on this machine in the background.
    fn keep_password(self: Pin<&mut Self>, password: String) {
        let seq = SECRET_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let _guard = SECRET_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            // A newer store or a forget came after this one.
            let result = if SECRET_SEQ.load(std::sync::atomic::Ordering::SeqCst) == seq {
                secret_store().store(&password).map(|_| ())
            } else {
                Ok(())
            };
            let _ = thread.queue(move |mut this| {
                if let Err(err) = result {
                    this.as_mut().set_error(format!("Passwort kann nicht gemerkt werden: {err:#}"));
                }
                this.as_mut().update_sync_state();
            });
        });
    }

    pub fn retry_password_command(mut self: Pin<&mut Self>) {
        reset_command();
        self.as_mut().fetch_password();
    }

    /// Runs the password command in the background, once per command line,
    /// while no password is known; synchronises when it succeeded.
    pub(super) fn fetch_password(mut self: Pin<&mut Self>) {
        let command = self.config.password_command.clone();
        let remembered = secret_store().kind().is_some();
        if !self.config.connected() || (command.is_empty() && !remembered) || password().is_some() {
            return;
        }
        // Marks this attempt (a changed command line tries again).
        let command = if command.is_empty() { "\u{0}remembered".to_owned() } else { command };
        {
            let Ok(mut c) = COMMAND.lock() else { return };
            if c.running || c.tried == command {
                return;
            }
            c.tried = command.clone();
            c.running = true;
            c.error.clear();
        }
        self.as_mut().update_sync_state();
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            // The remembered password first, then the command.
            let result = match secret_store().load() {
                Ok(Some((password, _))) => Ok(password),
                Ok(None) | Err(_) if !command.starts_with('\u{0}') => {
                    calendar::password_from_command(&command, COMMAND_TIMEOUT)
                }
                Ok(None) => Err(anyhow::anyhow!("kein gemerktes Passwort")),
                Err(err) => Err(err.context("Gemerktes Passwort")),
            };
            let _ = thread.queue(move |mut this| {
                // Only a changed setting (other command, remembered ⇄ command)
                // makes the result stale; a retry meanwhile does not.
                let current = this.config.password_command.clone();
                let stale = current != command && !(current.is_empty() && command.starts_with('\u{0}'));
                let mut ok = false;
                if let Ok(mut c) = COMMAND.lock() {
                    c.running = false;
                    if !stale {
                        c.tried = command;
                        match result {
                            Ok(password) => {
                                ok = true;
                                if let Ok(mut p) = PASSWORD.lock() {
                                    p.get_or_insert(password);
                                }
                            }
                            Err(err) => {
                                log::warn!("password command failed: {err:#}");
                                c.error = format!("{err:#}");
                            }
                        }
                    }
                }
                this.as_mut().update_sync_state();
                if ok {
                    this.sync_now();
                } else if stale {
                    this.fetch_password();
                }
            });
        });
    }

    pub fn set_password(mut self: Pin<&mut Self>, password: &QString) {
        let password = password.to_string();
        if let Ok(mut p) = PASSWORD.lock() {
            *p = (!password.is_empty()).then(|| password.clone());
        }
        if !password.is_empty() && self.config.remember_password {
            self.as_mut().keep_password(password);
        }
        self.as_mut().update_sync_state();
        self.sync_now();
    }

    pub fn google_sign_in(mut self: Pin<&mut Self>, client_id: &QString, client_secret: &QString) {
        let oauth = google::OAuthClient {
            id: client_id.to_string().trim().to_owned(),
            secret: client_secret.to_string().trim().to_owned(),
        };
        if oauth.id.is_empty() || oauth.secret.is_empty() {
            self.google_signed_in(false, qs("Client-ID und Client-Secret eintragen"));
            return;
        }
        // The browser comes back to this port (loopback redirect).
        let listener = match std::net::TcpListener::bind("127.0.0.1:0") {
            Ok(l) => l,
            Err(err) => {
                self.google_signed_in(false, qs(format!("Kein lokaler Port frei: {err}")));
                return;
            }
        };
        let redirect = match listener.local_addr() {
            Ok(a) => format!("http://127.0.0.1:{}", a.port()),
            Err(err) => {
                self.google_signed_in(false, qs(err.to_string()));
                return;
            }
        };
        let (state, verifier) = (google::random_token(), google::random_token());
        let endpoints = google::Endpoints::default();
        self.as_mut().google_sign_in_url(qs(google::auth_url(&endpoints, &oauth, &redirect, &state, &verifier)));
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = (|| -> anyhow::Result<(google::Tokens, Vec<Value>)> {
                let code = google::wait_for_code(&listener, &state, std::time::Duration::from_secs(300))?;
                let http = Client::new(Account::default(), "", true);
                let tokens = google::exchange_code(
                    |body| http.post_form(&endpoints.token, body).map_err(|e| anyhow::anyhow!("{e}")),
                    &oauth,
                    &code,
                    &redirect,
                    &verifier,
                )?;
                anyhow::ensure!(!tokens.email.is_empty(), "Google hat keine E-Mail-Adresse geschickt");
                let remote = Client::google(&tokens.email, oauth.clone(), &tokens.refresh, endpoints, true);
                let found = remote.discover().map_err(|e| anyhow::anyhow!("{e}"))?;
                let list = found
                    .calendars
                    .iter()
                    .map(|c| json!({ "url": c.url, "name": c.name, "color": c.color, "writable": c.writable }))
                    .collect();
                Ok((tokens, list))
            })();
            let _ = thread.queue(move |mut this| match result {
                Ok((tokens, list)) => {
                    this.as_mut().use_google_account(&oauth, &tokens.email, &tokens.refresh);
                    let message = format!("Angemeldet als {} – {} Kalender gefunden", tokens.email, list.len());
                    this.as_mut().connection_tested(true, qs(&message), qs(Value::from(list).to_string()));
                    this.google_signed_in(true, qs(message));
                }
                Err(err) => this.google_signed_in(false, qs(format!("{err:#}"))),
            });
        });
    }

    /// After the sign-in: the Google account in `config.toml`, the refresh
    /// token remembered (encrypted if possible) and for this Atlas start.
    fn use_google_account(mut self: Pin<&mut Self>, oauth: &google::OAuthClient, email: &str, refresh: &str) {
        let mut config = Config::load().calendar;
        if !(config.is_google() && config.user == email) {
            // Another account: its sign-in, calendars and cache go.
            let _ = forget_secret();
            let _ = self.sync_store().clear();
            config = CalendarConfig::default();
        }
        config.provider = "google".into();
        config.user = email.to_owned();
        config.google_client_id = oauth.id.clone();
        config.google_client_secret = oauth.secret.clone();
        // Without it every start would need the browser again.
        config.remember_password = true;
        if let Err(err) = Config::save_calendar(&config) {
            self.as_mut().set_error(format!("Einstellungen können nicht gespeichert werden: {err:#}"));
            return;
        }
        reset_command();
        if let Ok(mut p) = PASSWORD.lock() {
            *p = Some(refresh.to_owned());
        }
        self.as_mut().keep_password(refresh.to_owned());
        self.as_mut().reload();
        self.update_sync_state();
    }

    pub fn set_provider(mut self: Pin<&mut Self>, provider: &QString) {
        let provider = provider.to_string();
        let old = Config::load().calendar;
        if old.provider == provider {
            return;
        }
        if let Ok(mut p) = PASSWORD.lock() {
            *p = None;
        }
        reset_command();
        // (The store of the old provider, before the switch.)
        let _ = forget_secret();
        let config = CalendarConfig {
            provider,
            google_client_id: old.google_client_id,
            google_client_secret: old.google_client_secret,
            ..Default::default()
        };
        if let Err(err) = Config::save_calendar(&config) {
            self.as_mut().set_error(format!("Einstellungen können nicht gespeichert werden: {err:#}"));
        }
        let _ = self.sync_store().clear();
        self.reload();
    }

    pub fn disconnect_account(mut self: Pin<&mut Self>) {
        if let Ok(mut p) = PASSWORD.lock() {
            *p = None;
        }
        reset_command();
        let _ = forget_secret();
        if let Err(err) = Config::save_calendar(&CalendarConfig::default()) {
            self.as_mut().set_error(format!("Einstellungen können nicht gespeichert werden: {err:#}"));
        }
        let _ = self.sync_store().clear();
        self.reload();
    }
}
