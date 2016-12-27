#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}

pub mod houselights {
    extern crate sacn;
    use self::sacn::DmxSource;

    const UNIVERSE_SIZE: usize = 510;

    const GAMMA: f32 = 2.2;
    const GC_BOTTOM_THRESHOLD: u16 = 20;

    #[derive(Clone,Debug)]
    pub struct RGB {
        pub red:   u8,
        pub green: u8,
        pub blue:  u8
    }
    impl RGB {
        pub fn null() -> RGB {
            RGB { red: 0, green: 0, blue: 0 }
        }
    }

    #[derive(Clone,Debug)]
    pub struct HSV {
        pub hue:        f32,  // 0-360, degrees
        pub saturation: f32,  // 0-1
        pub brightness: f32   // 0-1
    }
    impl HSV {
        pub fn null() -> HSV {
            HSV {
                hue:        0_f32,
                saturation: 0_f32,
                brightness: 0_f32
            }
        }
    }

    #[derive(Clone,Debug)]
    pub struct Zone  {
        pub head: u8,
        pub body: u8,
        pub tail: u8,
        pub name: String
    }

    pub struct Dmx {
        source: DmxSource
    }
    impl Dmx {
        pub fn new() -> Dmx {
            Dmx { source: DmxSource::new("Controller").unwrap() }
        }
    }
    impl Drop for Dmx {
        fn drop(&mut self) {
            let _res = self.source.terminate_stream(1);
        }
    }
    
    // return RGB for a given color temperature
    pub fn kelvin (mut temp: u16) -> RGB {
        // http://www.tannerhelland.com/4435/convert-temperature-rgb-algorithm-code/
        temp /= 100;

        let mut rgb: RGB = RGB::null();
        // calculate red
        if temp <= 66 {
            rgb.red = 255;
        } else {
            let red: f32 = (temp - 60) as f32;
            rgb.red = _normalize_value((329.698727446 * red.powf(-0.1332047592)).round());
        }
        // calculate green
        if temp <= 66 {
            let green: f32 = temp as f32;
            rgb.green = _normalize_value((99.4708025861 * green.ln() - 161.1195681661).round());
        } else {
            let green: f32 = (temp - 60) as f32;
            rgb.green = _normalize_value((288.1221695283 * green.powf(-0.0755148492)).round())
        }
        // calculate blue
        if temp >= 66 {
            rgb.blue = 255;
        } else {
            if temp <= 19 {
                rgb.blue = 0;
            } else {
                let blue: f32 = (temp - 10) as f32;
                rgb.blue = _normalize_value((138.5177312231 * blue.ln() - 305.0447927307).round());
            }
        }
        return rgb;
    }

    fn _normalize_value (value: f32) -> u8 {
        if value < 0_f32 {
            return 0_u8;
        }
        if value > 255_f32 {
            return 255_u8;
        }
        return value as u8;
    }

    pub fn hsv2rgb(hsv: &HSV) -> RGB {
        let c = hsv.brightness * hsv.saturation;
        let x = c * (1_f32 - (((hsv.hue * 6_f32) % 2_f32).abs() - 1_f32));
        let m = hsv.brightness - c;
        let mut red   = 0_f32;
        let mut green = 0_f32;
        let mut blue  = 0_f32;
        if hsv.hue < 1_f32/6_f32 {
            red   = c;
            green = x;
        } else if hsv.hue < 1_f32/3_f32 {
            red   = x;
            green = c;
        } else if hsv.hue < 1_f32/2_f32 {
            green = c;
            blue  = x;
        } else if hsv.hue < 2_f32/3_f32 {
            green = x;
            blue  = c;
        } else if hsv.hue < 5_f32/6_f32 {
            red   = x;
            blue  = c;
        } else {
            red   = c;
            blue  = x;
        }
        let rgb = RGB {
            red:   ((red   + m) * 255_f32).round() as u8,
            green: ((green + m) * 255_f32).round() as u8,
            blue:  ((blue  + m) * 255_f32).round() as u8
        };
        return rgb;
    }

    pub fn scale_rgb(rgb: &RGB, intensity: f32, max_intensity: f32) -> RGB {
        let i: f32 = intensity * max_intensity as f32;
        let scaled: RGB = RGB {
            red:   (rgb.red   as f32 * i).round() as u8,
            green: (rgb.green as f32 * i).round() as u8,
            blue:  (rgb.blue  as f32 * i).round() as u8
        };
        return scaled;
    }
    
    pub fn gamma_correct(rgb: &RGB) -> RGB {
        let mut c: RGB = RGB {
            red:   (255_f32 * (rgb.red   as f32 / 255_f32).powf(GAMMA)) as u8,
            green: (255_f32 * (rgb.green as f32 / 255_f32).powf(GAMMA)) as u8,
            blue:  (255_f32 * (rgb.blue  as f32 / 255_f32).powf(GAMMA)) as u8
        };
        // drop to dark if sum of all thre falls below threshold
        // trying to avoid bottoming out to dim red or green when I want white
        if (c.red as u16 + c.green as u16 + c.blue as u16) < GC_BOTTOM_THRESHOLD {
            c.red   = 0;
            c.green = 0;
            c.blue  = 0;
        }
        return c;
    }
    
    pub fn render( lights: &[RGB], zones: &[Zone], dmx: &Dmx ) {
        let spliced = splice_null_pixels(lights, zones);
        let mut out: Vec<u8> = vec![];
        for rgb in spliced.iter() {
            let gc = gamma_correct(rgb);
            out.push(gc.red);
            out.push(gc.green);
            out.push(gc.blue);
        }
        let mut universes = Vec::new();
        while out.len() > UNIVERSE_SIZE {
            let u = out.split_off(UNIVERSE_SIZE);
            universes.push(out);
            out = u;
        }
        universes.push(out);
        let mut universe: u16 = 1;
        for u in universes {
            let _res = dmx.source.send(universe, &u);
            universe += 1;
        }
    }

    fn splice_null_pixels( lights: &[RGB], zones: &[Zone] ) -> Vec<RGB> {
        let mut copy: Vec<RGB> = vec![];
        copy.extend_from_slice(lights);
        let mut idx: usize = 0;
        for zone in zones {
            // null pixels at the head of the zone
            for _i in 0..zone.head {
                copy.insert(idx, RGB::null());
            }
            // and at the tail
            idx += zone.head as usize + zone.body as usize;
            for _i in 0..zone.tail {
                copy.insert(idx, RGB::null());
            }
            idx += zone.tail as usize;
        }
        return copy;
    }
}
