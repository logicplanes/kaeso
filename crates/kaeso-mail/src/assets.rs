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

pub const WORDMARKS: [Image; 2] = [WORDMARK_LIGHT, WORDMARK_DARK];

macro_rules! cover {
    ($name:literal) => {
        Image {
            cid: concat!("cover-", $name, "@kaeso"),
            width: 560,
            height: 240,
            png: include_bytes!(concat!("../assets/cover-", $name, ".png")),
        }
    };
}

pub const COVER_RELEASE: Image = cover!("release");
pub const COVER_WELCOME: Image = cover!("welcome");
pub const COVER_SUMMARY: Image = cover!("summary");
pub const COVER_COSTS: Image = cover!("costs");
pub const COVER_APPS: Image = cover!("apps");

pub const COVERS: [Image; 5] = [
    COVER_RELEASE,
    COVER_WELCOME,
    COVER_SUMMARY,
    COVER_COSTS,
    COVER_APPS,
];

#[cfg(test)]
mod tests {

    use super::*;

    const WORDMARK_SCALE: u32 = 3;
    const COVER_SCALE: u32 = 2;

    fn render(svg: &str, width: u32) -> Vec<u8> {
        let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).unwrap();
        let scale = width as f32 / tree.size().width();
        let size = tree.size().to_int_size().scale_by(scale).unwrap();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height()).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );
        pixmap.encode_png().unwrap()
    }

    fn name(image: &Image) -> &'static str {
        image.cid.trim_end_matches("@kaeso")
    }

    fn size(png: &[u8]) -> (u32, u32) {
        (
            u32::from_be_bytes(png[16..20].try_into().unwrap()),
            u32::from_be_bytes(png[20..24].try_into().unwrap()),
        )
    }

    #[test]
    fn the_pngs_are_the_svgs_rendered() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");
        let wordmarks = WORDMARKS.map(|i| (i, WORDMARK_SCALE));
        let covers = COVERS.map(|i| (i, COVER_SCALE));
        for (image, scale) in wordmarks.into_iter().chain(covers) {
            let name = name(&image);
            let svg = std::fs::read_to_string(format!("{dir}/{name}.svg")).unwrap();
            let png = render(&svg, image.width * scale);
            if std::env::var_os("KAESO_UPDATE_MAIL_ASSETS").is_some() {
                std::fs::write(format!("{dir}/{name}.png"), &png).unwrap();
                continue;
            }
            assert!(
                png == image.png,
                "assets/{name}.png is not assets/{name}.svg rendered; run the tests with KAESO_UPDATE_MAIL_ASSETS=1"
            );
        }
    }

    #[test]
    fn wordmarks_are_three_times_their_display_size_and_small() {
        for image in WORDMARKS {
            let (w, h) = size(image.png);
            assert_eq!((w, h), (image.width * 3, image.height * 3), "{}", image.cid);
            assert!(image.png.len() < 8 * 1024, "{}", image.cid);
        }
    }

    #[test]
    fn covers_are_twice_their_display_size_and_under_60_kib() {
        for image in COVERS {
            let (w, h) = size(image.png);
            assert_eq!((w, h), (image.width * 2, image.height * 2), "{}", image.cid);
            assert!(
                image.png.len() < 60 * 1024,
                "{}: {} bytes",
                image.cid,
                image.png.len()
            );
        }
    }

    #[test]
    fn every_image_has_its_own_content_id() {
        let all: Vec<&str> = WORDMARKS.iter().chain(&COVERS).map(|i| i.cid).collect();
        for cid in &all {
            assert_eq!(all.iter().filter(|c| *c == cid).count(), 1, "{cid}");
        }
    }
}
