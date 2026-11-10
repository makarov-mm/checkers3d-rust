//! What the game asks of the system around it: a window with an OpenGL context, the folder for its settings,
//! the user's language, the program's icon, and a way to report a failure to start.

use std::ffi::CString;
use std::num::NonZeroU32;
use std::path::PathBuf;

use glutin::config::{ConfigTemplateBuilder, GlConfig};
use glutin::context::{ContextApi, ContextAttributesBuilder, GlProfile, NotCurrentGlContext, PossiblyCurrentContext, Version};
use glutin::display::{GetGlDisplay, GlDisplay};
use glutin::surface::{GlSurface, Surface, SwapInterval, WindowSurface};
use glutin_winit::{DisplayBuilder, GlWindow as _};
use raw_window_handle::HasWindowHandle;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Icon, Window, WindowAttributes};

use crate::Result;
use crate::gfx;

/// A window and its OpenGL 4.6 (or at least 4.5) core context, current on the calling thread.
pub struct GlWindow {
    // The surface and the context go before the window they belong to.
    surface: Surface<WindowSurface>,
    context: PossiblyCurrentContext,
    pub window: Window,
    version: i32,
}

impl GlWindow {
    pub fn new(event_loop: &ActiveEventLoop, attributes: WindowAttributes) -> Result<GlWindow> {
        // The scene is drawn into textures of its own; the window needs color only, without multisampling.
        let template = ConfigTemplateBuilder::new().with_alpha_size(8).with_depth_size(24).with_stencil_size(8);
        let fewest_samples = |configs: Box<dyn Iterator<Item = glutin::config::Config> + '_>| {
            configs.min_by_key(|config| config.num_samples()).expect("the display offers at least one pixel format")
        };
        let (window, config) = DisplayBuilder::new()
            .with_window_attributes(Some(attributes))
            .build(event_loop, template, fewest_samples)
            .map_err(|error| format!("The window has no OpenGL pixel format: {error}"))?;
        let window = window.ok_or("The window could not be created.")?;
        let display = config.display();
        let handle = window.window_handle()?.as_raw();

        // OpenGL 4.6 core, or 4.5 where the driver stops there.
        let context = [6, 5]
            .into_iter()
            .find_map(|minor| {
                let attributes = ContextAttributesBuilder::new()
                    .with_context_api(ContextApi::OpenGl(Some(Version::new(4, minor))))
                    .with_profile(GlProfile::Core)
                    .with_debug(cfg!(debug_assertions))
                    .build(Some(handle));
                unsafe { display.create_context(&config, &attributes) }.ok()
            })
            .ok_or("OpenGL 4.5 or newer is not available. Please update the graphics driver.")?;

        let surface_attributes = window.build_surface_attributes(Default::default())?;
        let surface = unsafe { display.create_window_surface(&config, &surface_attributes)? };
        let context = context.make_current(&surface)?;
        gl::load_with(|name| CString::new(name).map_or(std::ptr::null(), |name| display.get_proc_address(&name)));

        // A sample of the functions newer than OpenGL 4.1 that the game cannot do without.
        let required = [
            ("glCreateTextures", gl::CreateTextures::is_loaded()),
            ("glNamedBufferStorage", gl::NamedBufferStorage::is_loaded()),
            ("glBindTextureUnit", gl::BindTextureUnit::is_loaded()),
            ("glProgramUniform4fv", gl::ProgramUniform4fv::is_loaded()),
            ("glDispatchCompute", gl::DispatchCompute::is_loaded()),
            ("glBindImageTexture", gl::BindImageTexture::is_loaded()),
        ];
        let missing: Vec<&str> = required.iter().filter(|(_, loaded)| !loaded).map(|(name, _)| *name).collect();
        if let Some(first) = missing.first() {
            return Err(format!("The graphics driver lacks {} OpenGL functions, such as {first}.", missing.len()).into());
        }

        // Swapping the buffers waits for the screen, where the driver agrees to.
        let _ = surface.set_swap_interval(&context, SwapInterval::Wait(NonZeroU32::MIN));
        if cfg!(debug_assertions) {
            gfx::report_errors();
        }

        let version = gfx::get_integer(gl::MAJOR_VERSION) * 10 + gfx::get_integer(gl::MINOR_VERSION);
        Ok(GlWindow { surface, context, window, version })
    }

    /// 46 for OpenGL 4.6.
    pub fn version(&self) -> i32 {
        self.version
    }

    pub fn swap_buffers(&self) {
        let _ = self.surface.swap_buffers(&self.context);
    }

    /// Follows the window's size; some systems size the surface apart from the window.
    pub fn resize(&self, width: u32, height: u32) {
        if let (Some(width), Some(height)) = (NonZeroU32::new(width), NonZeroU32::new(height)) {
            self.surface.resize(&self.context, width, height);
        }
    }
}

/// The folder for the program's settings, made if it is not there:
/// `%APPDATA%\<name>`, `~/Library/Application Support/<name>` or `$XDG_CONFIG_HOME/<name>`.
pub fn config_folder(name: &str) -> PathBuf {
    let variable = |key: &str| std::env::var_os(key).filter(|value| !value.is_empty()).map(PathBuf::from);
    let base = if cfg!(windows) {
        variable("APPDATA")
    } else if cfg!(target_os = "macos") {
        variable("HOME").map(|home| home.join("Library/Application Support"))
    } else {
        variable("XDG_CONFIG_HOME").or_else(|| variable("HOME").map(|home| home.join(".config")))
    };
    let folder = base.unwrap_or_else(|| PathBuf::from(".")).join(name);
    let _ = std::fs::create_dir_all(&folder);
    folder
}

/// Whether the system speaks Russian, Ukrainian or Belarusian: then the interface starts in Russian.
pub fn prefers_russian() -> bool {
    sys_locale::get_locale().is_some_and(|locale| {
        let language = locale.split(['-', '_']).next().unwrap_or_default().to_ascii_lowercase();
        matches!(language.as_str(), "ru" | "uk" | "be")
    })
}

/// The program's icon for the title bar and the task bar: on Windows the one built into the executable.
#[cfg(windows)]
pub fn window_icon() -> Option<Icon> {
    use winit::platform::windows::IconExtWindows;
    Icon::from_resource(1, None).ok()
}

/// The program's icon: the largest picture of the icon file that window managers take gladly.
#[cfg(all(unix, not(target_os = "macos")))]
pub fn window_icon() -> Option<Icon> {
    const ICON: &[u8] = include_bytes!("../assets/Checkers3D.ico");
    const WANTED: u32 = 64;

    // The file lists its pictures, each a PNG: one byte each for the width and the height, and at the end of
    // the 16-byte entry the picture's size and place in the file.
    let count = u16::from_le_bytes(ICON.get(4..6)?.try_into().ok()?) as usize;
    let width = |entry: &[u8]| if entry[0] == 0 { 256 } else { u32::from(entry[0]) };
    let entry = ICON.get(6..)?.chunks_exact(16).take(count).min_by_key(|entry| width(entry).abs_diff(WANTED))?;
    let size = u32::from_le_bytes(entry[8..12].try_into().ok()?) as usize;
    let offset = u32::from_le_bytes(entry[12..16].try_into().ok()?) as usize;
    let mut reader = png::Decoder::new(std::io::Cursor::new(ICON.get(offset..offset + size)?)).read_info().ok()?;
    let mut pixels = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut pixels).ok()?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    pixels.truncate(info.buffer_size());
    Icon::from_rgba(pixels, info.width, info.height).ok()
}

/// The icon of a macOS program comes from its bundle.
#[cfg(target_os = "macos")]
pub fn window_icon() -> Option<Icon> {
    None
}

/// Tells the user why the game could not start.
#[cfg(windows)]
pub fn show_error(title: &str, message: &str) {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(window: *mut std::ffi::c_void, text: *const u16, caption: *const u16, kind: u32) -> i32;
    }
    const MB_ICONERROR: u32 = 0x10;

    let wide = |text: &str| std::ffi::OsStr::new(text).encode_wide().chain(Some(0)).collect::<Vec<u16>>();
    let (message, title) = (wide(message), wide(title));
    unsafe { MessageBoxW(std::ptr::null_mut(), message.as_ptr(), title.as_ptr(), MB_ICONERROR) };
}

#[cfg(not(windows))]
pub fn show_error(title: &str, message: &str) {
    eprintln!("{title}: {message}");
}
