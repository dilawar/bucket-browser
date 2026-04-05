use egui::{Align, Color32, Grid, Layout, RichText, TextEdit, Ui};

use crate::storage::{ENV_ACCESS_KEY, ENV_BUCKET, ENV_ENDPOINT, ENV_REGION, ENV_SECRET_KEY};

// ── ConfigFields ──────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct ConfigFields {
    /// Primary input — encodes bucket + endpoint + region as a URI.
    /// e.g. `s3://bucket/` or `s3://bucket/?endpoint=https%3A%2F%2F…&region=…`
    pub connection_uri: String,
    // Derived / individually editable fields (kept in sync with connection_uri).
    pub bucket: String,
    pub endpoint: String, // raw, unencoded; blank = AWS default
    pub region: String,
    // Credentials — never encoded into the URI.
    pub access_key: String,
    pub secret_key: String,
    pub remember: bool,
}

impl ConfigFields {
    /// Load initial values from env vars, then saved credentials, then compute URI.
    pub fn load() -> Self {
        let from_env = |var: &str| std::env::var(var).unwrap_or_default();

        let mut f = Self {
            bucket: from_env(ENV_BUCKET),
            endpoint: from_env(ENV_ENDPOINT),
            access_key: from_env(ENV_ACCESS_KEY),
            secret_key: from_env(ENV_SECRET_KEY),
            region: from_env(ENV_REGION),
            ..Default::default()
        };

        if let Some(saved) = crate::credentials::CredentialStore::open()
            .ok()
            .and_then(|s| s.load())
        {
            if f.bucket.is_empty() {
                f.bucket = saved.bucket;
            }
            if f.endpoint.is_empty() {
                f.endpoint = saved.endpoint;
            }
            if f.access_key.is_empty() {
                f.access_key = saved.access_key;
            }
            if f.secret_key.is_empty() {
                f.secret_key = saved.secret_key;
            }
            if f.region.is_empty() {
                f.region = saved.region;
            }
            f.remember = true;
        }

        f.connection_uri = f.compute_uri();
        f
    }

    // ── URI ↔ fields ─────────────────────────────────────────────────────────

    /// Build a `s3://` URI from the current field values.
    pub fn compute_uri(&self) -> String {
        let bucket = self.bucket.trim();
        let endpoint = self.endpoint.trim();
        let region = self.region.trim();

        let mut params: Vec<String> = Vec::new();
        if !endpoint.is_empty() {
            params.push(format!("endpoint={}", urlencoding::encode(endpoint)));
        }
        if !region.is_empty() {
            params.push(format!("region={}", urlencoding::encode(region)));
        }

        if params.is_empty() {
            format!("s3://{bucket}/")
        } else {
            format!("s3://{bucket}/?{}", params.join("&"))
        }
    }

    /// Parse `connection_uri` and update the individual fields.
    /// Handles two input forms:
    /// - `s3://bucket/?endpoint=<encoded>&region=<region>`
    /// - `https://endpoint/bucket`  (also `http://`)
    pub fn parse_uri_into_fields(&mut self) {
        let uri = self.connection_uri.trim().to_owned();

        if let Some(rest) = uri.strip_prefix("s3://") {
            let (authority, query) = match rest.split_once('?') {
                Some((a, q)) => (a, q),
                None => (rest, ""),
            };
            self.bucket = authority.split('/').next().unwrap_or("").to_owned();
            self.endpoint.clear();
            self.region.clear();
            for pair in query.split('&').filter(|s| !s.is_empty()) {
                if let Some((k, v)) = pair.split_once('=') {
                    let decoded = urlencoding::decode(v)
                        .map(|c| c.into_owned())
                        .unwrap_or_else(|_| v.to_owned());
                    match k {
                        "endpoint" => self.endpoint = decoded,
                        "region" => self.region = decoded,
                        _ => {}
                    }
                }
            }
        } else {
            // Accept `https://endpoint/bucket` and `http://endpoint/bucket`.
            let (scheme, rest) = if let Some(r) = uri.strip_prefix("https://") {
                ("https", r)
            } else if let Some(r) = uri.strip_prefix("http://") {
                ("http", r)
            } else {
                return; // unrecognised scheme — leave fields unchanged
            };

            let mut parts = rest.splitn(2, '/');
            let host = parts.next().unwrap_or("");
            let bucket = parts.next().unwrap_or("").trim_end_matches('/');

            self.endpoint = format!("{scheme}://{host}");
            if !bucket.is_empty() {
                self.bucket = bucket.to_owned();
            }

            // Auto-extract region from Backblaze B2 hostnames:
            // s3.<region>.backblazeb2.com
            if let Some(inner) = host.strip_suffix(".backblazeb2.com")
                && let Some(region) = inner.strip_prefix("s3.")
            {
                self.region = region.to_owned();
            }
        }
    }

    /// The endpoint value to pass to `S3Backend::with_credentials`.
    /// Returns `None` for plain AWS (blank endpoint).
    pub fn resolved_endpoint(&self) -> Option<String> {
        let ep = self.endpoint.trim();
        if ep.is_empty() {
            None
        } else {
            Some(ep.to_owned())
        }
    }
}

// ── UI ────────────────────────────────────────────────────────────────────────

pub struct ConfigResponse {
    pub connect: bool,
}

pub fn show(ui: &mut Ui, f: &mut ConfigFields, error: Option<&str>) -> ConfigResponse {
    let mut connect = false;

    // Push the form down so it appears vertically centred.
    // 370 px is a reasonable estimate of the form's rendered height.
    let v_pad = ((ui.available_height() - 370.0) / 2.0).max(16.0);
    ui.add_space(v_pad);

    ui.vertical_centered(|ui| {
        ui.set_max_width(560.0);
        ui.add_space(4.0);
        ui.heading("Connect to S3-compatible storage");
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "Paste a connection URI, or fill the fields below.  \
                 Credentials are kept separate.",
            )
            .size(13.0)
            .color(Color32::from_gray(90)),
        );
        ui.add_space(12.0);

        let mut uri_changed    = false;
        let mut fields_changed = false;

        // ── Connection + secondary fields ─────────────────────────────────────
        Grid::new("config_uri_grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Connection URI *");
                let r = ui.add(
                    TextEdit::singleline(&mut f.connection_uri)
                        .hint_text("s3://bucket/  ·  https://endpoint/bucket  ·  s3://bucket/?endpoint=…&region=…")
                        .desired_width(420.0)
                        .font(egui::TextStyle::Monospace),
                );
                if r.changed() { uri_changed = true; }
                ui.end_row();

                ui.label("Bucket *");
                if ui.add(
                    TextEdit::singleline(&mut f.bucket)
                        .hint_text("my-bucket")
                        .desired_width(280.0),
                ).changed() { fields_changed = true; }
                ui.end_row();

                ui.label("Endpoint");
                if ui.add(
                    TextEdit::singleline(&mut f.endpoint)
                        .hint_text("https://… (leave blank for AWS S3)"),
                ).changed() { fields_changed = true; }
                ui.end_row();

                ui.label("Region");
                if ui.add(
                    TextEdit::singleline(&mut f.region)
                        .hint_text("us-east-1"),
                ).changed() { fields_changed = true; }
                ui.end_row();
            });

        // Bidirectional sync — only one side can change per frame.
        if uri_changed {
            f.parse_uri_into_fields();
        } else if fields_changed {
            f.connection_uri = f.compute_uri();
        }

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);

        // ── Credentials ───────────────────────────────────────────────────────
        Grid::new("config_creds_grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Access Key *");
                ui.add(
                    TextEdit::singleline(&mut f.access_key)
                        .hint_text("AKIAIOSFODNN7EXAMPLE")
                        .desired_width(280.0),
                );
                ui.end_row();

                ui.label("Secret Key *");
                ui.add(
                    TextEdit::singleline(&mut f.secret_key)
                        .password(true)
                        .hint_text("wJalrXUtnFEMI/K7MDENG/…"),
                );
                ui.end_row();

                ui.label("");
                ui.checkbox(&mut f.remember, "Remember credentials (encrypted)");
                ui.end_row();
            });

        ui.add_space(8.0);

        if let Some(msg) = error {
            ui.label(
                RichText::new(format!("✗  {msg}"))
                    .color(Color32::from_rgb(180, 30, 30))
                    .size(13.0),
            );
            ui.add_space(4.0);
        }

        let ready = !f.bucket.is_empty()
            && !f.access_key.is_empty()
            && !f.secret_key.is_empty();

        ui.with_layout(Layout::top_down(Align::Center), |ui| {
            let btn = egui::Button::new(
                RichText::new("  Connect  ")
                    .strong()
                    .size(16.0)
                    .color(Color32::WHITE),
            )
            .fill(if ready {
                Color32::from_rgb(37, 99, 235)
            } else {
                Color32::from_rgb(100, 120, 160)
            })
            .corner_radius(6.0)
            .min_size(egui::vec2(160.0, 38.0));

            if ui.add_enabled(ready, btn).clicked() {
                connect = true;
            }
        });

        ui.add_space(8.0);
        ui.label(
            RichText::new("Set AWS_S3_BUCKET + credential env vars to skip this screen.")
                .size(13.0)
                .color(Color32::from_gray(90)),
        );
    });

    ConfigResponse { connect }
}
