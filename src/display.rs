use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use embedded_svc::storage::Storage;
use esp_idf_hal::gpio::OutputPin;
use esp_idf_hal::rmt::HwChannel;
use smart_leds::{SmartLedsWrite, RGB8};
use static_cell::StaticCell;
use tracing::{error, info};

use crate::bluetooth::{CURRENT_MESSAGE, MESSAGE_UPDATED};
use crate::dither::GammaDither;
use crate::font::{self, ScrollingRender};
use crate::leds;
use crate::storage::STORAGE;

fn rgb(x: u8, y: u8, offs: u8) -> RGB8 {
    fn conv_colour(c: cichlid::ColorRGB) -> smart_leds::RGB8 {
        smart_leds::RGB8::new(c.r, c.g, c.b)
    }

    let v = cichlid::HSV {
        h: ((y / 4) as u8).wrapping_add(x * 10).wrapping_add(offs),
        s: 200,
        v: 130,
    };

    conv_colour(v.to_rgb_rainbow())
}

pub fn led_task(
    heart: Arc<AtomicBool>,
    pin: impl OutputPin,
    rmt: impl HwChannel,
) -> color_eyre::Result<()> {
    static MEM: StaticCell<leds::Esp32NeopixelMem<25>> = StaticCell::new();
    let mem = MEM.init_with(leds::Esp32NeopixelMem::<25>::new);
    let mut leds = leds::Esp32Neopixel::<_, _, 25>::new(pin, rmt, mem)?;

    let mut i = 0u8;

    let mut message = if let Ok(Some(text)) = STORAGE
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .get::<String>("message")
    {
        ScrollingRender::from_str(text.as_str())?
    } else {
        ScrollingRender::from_str("hello world")?
    };

    loop {
        if heart.load(std::sync::atomic::Ordering::Relaxed) {
            let it = font::FONT[0x3]
                .mask_with_x_offset(
                    0,
                    leds::with_positions(|_x, _y| RGB8::new(0xFD, 0x3F, 0x92)),
                )
                .map(|(_, v)| v.unwrap_or(RGB8::new(0, 0, 0)));

            let _ = leds.write(GammaDither::<1, 15>::dither(0, it));
        } else {
            let it = message.render(|x, y| rgb(x, y, i as u8));

            let _ = leds.write(GammaDither::<1, 15>::dither(0, it));
        }

        std::thread::sleep(Duration::from_millis(33 * 3));
        i = i.wrapping_add(1);
        if message.step() {
            info!("message done! updating to a new one");
            if MESSAGE_UPDATED.swap(false, std::sync::atomic::Ordering::SeqCst) {
                let text = CURRENT_MESSAGE.lock().unwrap();

                STORAGE
                    .get()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .set("message", &text.as_str())?;

                match ScrollingRender::from_str(text.as_str()) {
                    Ok(m) => {
                        message = m;
                    }
                    Err(err) => {
                        error!(?err, "Failed to update message");
                    }
                }
            }
        }
    }
}
