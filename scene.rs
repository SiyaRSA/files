use vello::kurbo::{Affine, Rect};
use vello::peniko::{Color, Fill};
use crate::editor::Size;
use vello::Scene;


pub struct AppScene;

impl AppScene {
    pub fn build(size: Size) -> Scene {
        let mut scene = Scene::new();

        let width = size.width as f64;
        let height = size.height as f64;

        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            Color::from_rgb8(46, 46, 46),
            None,
            &Rect::from_origin_size(
                (0.0, 0.0),
                (width, height))
        );

        scene
    }
}