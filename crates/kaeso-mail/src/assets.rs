#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Image {
    pub cid: &'static str,
    pub width: u32,
    pub height: u32,
    pub png: &'static [u8],
}

pub const WORDMARK_LIGHT: Image = Image {
    cid: "wordmark-light@kaeso",
    width: 64,
    height: 16,
    png: include_bytes!("../assets/wordmark-light.png"),
};

pub const WORDMARK_DARK: Image = Image {
    cid: "wordmark-dark@kaeso",
    width: 64,
    height: 16,
    png: include_bytes!("../assets/wordmark-dark.png"),
};

pub const ALL: [Image; 2] = [WORDMARK_LIGHT, WORDMARK_DARK];

#[cfg(test)]
mod tests {

    use super::*;

    const SCALE: f32 = 3.0;

    fn render(svg: &str) -> Vec<u8> {
        let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).unwrap();
        let size = tree.size().to_int_size().scale_by(SCALE).unwrap();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height()).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(SCALE, SCALE),
            &mut pixmap.as_mut(),
        );
        pixmap.encode_png().unwrap()
    }

    #[test]
    fn the_pngs_are_the_svgs_rendered() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");
        for (name, image) in [("light", WORDMARK_LIGHT), ("dark", WORDMARK_DARK)] {
            let svg = std::fs::read_to_string(format!("{dir}/wordmark-{name}.svg")).unwrap();
            let png = render(&svg);
            if std::env::var_os("KAESO_UPDATE_MAIL_ASSETS").is_some() {
                std::fs::write(format!("{dir}/wordmark-{name}.png"), &png).unwrap();
                continue;
            }
            assert!(
                png == image.png,
                "assets/wordmark-{name}.png is not assets/wordmark-{name}.svg rendered; run the tests with KAESO_UPDATE_MAIL_ASSETS=1"
            );
        }
    }

    #[test]
    fn the_pngs_are_three_times_their_display_size_and_small() {
        for image in ALL {
            let w = u32::from_be_bytes(image.png[16..20].try_into().unwrap());
            let h = u32::from_be_bytes(image.png[20..24].try_into().unwrap());
            assert_eq!((w, h), (image.width * 3, image.height * 3), "{}", image.cid);
            assert!(image.png.len() < 8 * 1024, "{}", image.cid);
        }
    }
}
