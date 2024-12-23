use embedded_svc::{
    http::{Headers, Method},
    io::{Read, Write},
};
use esp_idf_svc::http::server::EspHttpServer;
use esp_idf_svc::{
    hal::rmt::TxRmtDriver,
    sys::{heap_caps_get_free_size, MALLOC_CAP_DEFAULT},
};
use log::info;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{rgb::Rgb, rmt_neopixel::neopixel};

static INDEX_HTML: &str = include_str!("http_server_page.html");

// Max payload length
const MAX_LEN: usize = 128;

// Need lots of stack to parse JSON
const STACK_SIZE: usize = 10240;

#[derive(Deserialize)]
struct FormData<'a> {
    first_name: &'a str,
    age: u32,
    birthplace: &'a str,
    color: &'a str,
}

#[derive(Serialize)]
struct Debug {
    free_heap: usize,
}

pub fn create_http_server(
    rgb_handler: Arc<Mutex<TxRmtDriver<'static>>>,
    history: Arc<Mutex<HashMap<String, Vec<(i64, u16)>>>>,
) -> anyhow::Result<EspHttpServer<'static>> {
    let server_configuration = esp_idf_svc::http::server::Configuration {
        stack_size: STACK_SIZE,
        ..Default::default()
    };

    let mut server = EspHttpServer::new(&server_configuration)?;

    server.fn_handler("/", Method::Get, |req| {
        req.into_ok_response()?
            .write_all(INDEX_HTML.as_bytes())
            .map(|_| ())
    })?;

    server.fn_handler("/debug", Method::Get, move |req| {
        let free_heap = unsafe { heap_caps_get_free_size(MALLOC_CAP_DEFAULT) };

        let debug = Debug { free_heap };

        let b = serde_json::to_string(&debug).unwrap();
        req.into_ok_response()?.write_all(b.as_bytes()).map(|_| ())
    })?;

    server.fn_handler("/temp", Method::Get, move |req| {
        let data = &*history.lock().unwrap();

        let b = serde_json::to_string(data).unwrap();
        // dbg!(&b);
        req.into_ok_response()?.write_all(b.as_bytes()).map(|_| ())
    })?;

    server.fn_handler::<anyhow::Error, _>("/post", Method::Post, move |mut req| {
        let len = req.content_len().unwrap_or(0) as usize;

        if len > MAX_LEN {
            req.into_status_response(413)?
                .write_all("Request too big".as_bytes())?;
            return Ok(());
        }

        let mut buf = vec![0; len];
        req.read_exact(&mut buf)?;
        let mut resp = req.into_ok_response()?;

        dbg!(String::from_utf8_lossy(&buf));

        if let Ok(form) = serde_json::from_slice::<FormData>(&buf) {
            let mut tx = rgb_handler.lock().unwrap();
            neopixel(Rgb::from_hexa(&form.color)?, &mut tx)?;

            write!(
                resp,
                "Hello, {}-year-old {} from {} ({})!",
                form.age, form.first_name, form.birthplace, form.color
            )?;
        } else {
            info!("JSON error");
            resp.write_all("JSON error".as_bytes())?;
        }

        Ok(())
    })?;

    Ok(server)
}
