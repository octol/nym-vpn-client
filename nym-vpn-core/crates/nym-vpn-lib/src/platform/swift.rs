// Copyright 2023 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: GPL-3.0-only

use std::str::FromStr;

use log::LevelFilter;
use oslog::OsLogger;

pub fn init_logs(level: String) {
    let result = OsLogger::new("net.nymtech.vpn.agent")
        .level_filter(LevelFilter::from_str(&level).unwrap_or(LevelFilter::Info))
        .category_level_filter("hyper", LevelFilter::Warn)
        .category_level_filter("tokio_reactor", LevelFilter::Warn)
        .category_level_filter("reqwest", LevelFilter::Warn)
        .category_level_filter("mio", LevelFilter::Warn)
        .category_level_filter("want", LevelFilter::Warn)
        .category_level_filter("tungstenite", LevelFilter::Warn)
        .category_level_filter("tokio_tungstenite", LevelFilter::Warn)
        .category_level_filter("handlebars", LevelFilter::Warn)
        .category_level_filter("sled", LevelFilter::Warn)
        .init();

    match result {
        Ok(_) => {
            tracing::debug!("Logger initialized");
        }
        Err(e) => {
            tracing::error!("Failed to initialize os_log: {}", e);
        }
    };
}
