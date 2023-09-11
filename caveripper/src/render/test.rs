use paste::paste;
use rand::{rngs::SmallRng, Rng, SeedableRng};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::{assets::AssetManager, layout::Layout, render::*};

macro_rules! test_render {
    ($($name: literal),+) => {
        paste!{
            $(
                #[test]
                fn [<test_render_layouts_ $name>] () {
                    let mgr = AssetManager::init().unwrap();
                    let renderer = Renderer::new(&mgr);

                    let num_layouts = 4;
                    let caveinfos = mgr.caveinfos_from_cave($name.replace('_', ":").as_str()).unwrap();

                    caveinfos.into_par_iter().panic_fuse().for_each(|caveinfo| {
                        let mut rng: SmallRng = SeedableRng::seed_from_u64(0x12345678);
                        for _ in 0..num_layouts {
                            let seed: u32 = rng.gen();
                            let layout = Layout::generate(seed, &caveinfo);
                            if let Err(cause) = renderer.render_layout(&layout, LayoutRenderOptions::default()) {
                                panic!("({}, {:#010X}) {}", $name, seed, cause);
                            }
                        }
                    });
                }

                #[test]
                fn [<test_render_caveinfo_ $name>] () {
                    let mgr = AssetManager::init().unwrap();
                    let renderer = Renderer::new(&mgr);
                    let caveinfos = mgr.caveinfos_from_cave($name.replace('_', ":").as_str()).unwrap();
                    caveinfos.into_par_iter().panic_fuse().for_each(|caveinfo| {
                        renderer.render_caveinfo(&caveinfo, CaveinfoRenderOptions::default()).unwrap();
                    });
                }
            )*
        }
    }
}

test_render!(
    "ec",
    "scx",
    "fc",
    "hob",
    "wfg",
    "bk",
    "sh",
    "cos",
    "gk",
    "sr",
    "smc",
    "coc",
    "hoh",
    "dd",
    "exc",
    "nt",
    "ltb",
    "cg",
    "gh",
    "hh",
    "ba",
    "rc",
    "tg",
    "twg",
    "cc",
    "cm",
    "cr",
    "dn",
    "ca",
    "sp",
    "tct",
    "ht",
    "csn",
    "gb",
    "rg",
    "sl",
    "hg",
    "ad",
    "str",
    "bg",
    "cop",
    "bd",
    "snr",
    "er",
    "newyear_bg",
    "newyear_sk",
    "newyear_cwnn",
    "newyear_snd",
    "newyear_ch",
    "newyear_rh",
    "newyear_ss",
    "newyear_sa",
    "newyear_aa",
    "newyear_ser",
    "newyear_tc",
    "newyear_er",
    "newyear_cg",
    "newyear_sd",
    "newyear_ch1",
    "newyear_ch2",
    "newyear_ch3",
    "newyear_ch4",
    "newyear_ch5",
    "newyear_ch6",
    "newyear_ch7",
    "newyear_ch8",
    "newyear_ch9",
    "newyear_ch10",
    "newyear_ch11",
    "newyear_ch12",
    "newyear_ch13",
    "newyear_ch14",
    "newyear_ch15",
    "newyear_ch16",
    "newyear_ch17",
    "newyear_ch18",
    "newyear_ch19",
    "newyear_ch20",
    "newyear_ch21",
    "newyear_ch22",
    "newyear_ch23",
    "newyear_ch24",
    "newyear_ch25",
    "newyear_ch26",
    "newyear_ch27",
    "newyear_ch28",
    "newyear_ch29",
    "newyear_ch30",
    "251_at",
    "251_aqd",
    "251_ft",
    "251_gd",
    "251_gdd",
    "251_im",
    "251_sc",
    "251_wf",
    "251_aa",
    "251_ss",
    "251_ck",
    "251_potw",
    "251_ch1",
    "251_ch2",
    "251_ch3",
    "251_ch4",
    "251_ch5",
    "251_ch6",
    "251_ch7",
    "251_ch8",
    "251_ch9",
    "251_ch10",
    "251_ch11",
    "251_ch12",
    "251_ch13",
    "251_ch14",
    "251_ch15",
    "251_ch16",
    "251_ch17",
    "251_ch18",
    "251_ch19",
    "251_ch20",
    "251_ch21",
    "251_ch22",
    "251_ch23",
    "251_ch24",
    "251_ch25",
    "251_ch26",
    "251_ch27",
    "251_ch28",
    "251_ch29",
    "251_ch30",
    "bikmin_oh",
    "bikmin_ll",
    "bikmin_km",
    "bikmin_fzc",
    "bikmin_fof",
    "bikmin_bn",
    "bikmin_ht",
    "bikmin_fj",
    "bikmin_ba",
    "bikmin_vc",
    "bikmin_tah",
    "bikmin_jnw",
    "bikmin_af",
    "bikmin_rl"
);
