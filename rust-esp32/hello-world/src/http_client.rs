fn post_request(
    client: &mut HttpClient<EspHttpConnection>,
    data: BLEAdvertisedData,
) -> anyhow::Result<()> {
    // Prepare payload
    let binding = json!(data).to_string();
    let payload = binding.as_bytes();

    // Prepare headers and URL
    let content_length_header = format!("{}", payload.len());
    let headers = [
        ("content-type", "application/json"),
        ("content-length", &*content_length_header),
    ];
    let url = "http://192.168.1.129:8080/frame";

    // Send request
    let mut request = client.post(url, &headers)?;
    request.write_all(payload)?;
    request.flush()?;
    // info!("-> POST {}", url);
    let mut response = request.submit()?;

    // Process response
    // let status = response.status();
    // info!("<- {}", status);
    let mut buf = [0u8; 1024];
    let bytes_read = io::try_read_full(&mut response, &mut buf).map_err(|e| e.0)?;
    // info!("Read {} bytes", bytes_read);
    match std::str::from_utf8(&buf[0..bytes_read]) {
        Ok(body_string) => {
            // info!(
            //     "Response body (truncated to {} bytes): {:?}",
            //     buf.len(),
            //     body_string
            // )
        }
        Err(e) => error!("Error decoding response body: {}", e),
    };

    Ok(())
}
