use base64::{engine::general_purpose, Engine as _};
use screenshots::Screen;
use std::time::Instant;

#[tauri::command]
pub async fn capture_primary_screen() -> Result<String, String> {
    use std::io::Cursor;
    let start = Instant::now();
    let screens = Screen::all().map_err(|e| e.to_string())?;

    if let Some(screen) = screens.first() {
        let capture = screen.capture().map_err(|e| e.to_string())?;

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        capture
            .write_to(&mut cursor, screenshots::image::ImageFormat::Png)
            .map_err(|e: screenshots::image::ImageError| e.to_string())?;

        let b64 = general_purpose::STANDARD.encode(buffer);
        println!("Captured screen in {:?}", start.elapsed());
        Ok(b64)
    } else {
        Err("No screens found".to_string())
    }
}
