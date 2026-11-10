//! The OpenGL 4.6 core functions the game uses, behind safe wrappers, and the objects they make.
//!
//! The context is made current on the main thread before anything here is called and stays current for as
//! long as the window lives; the wrappers rely on that, and on being called from that thread only.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_void};
use std::sync::atomic::{AtomicU32, Ordering};

use gl::types::{GLchar, GLenum, GLint, GLsizei, GLsizeiptr, GLuint};

use crate::Result;
use crate::math::{Mat4, Vec2, Vec3, Vec4};

/// The bytes of plain values: structs of floats and integers without padding.
pub fn bytes_of<T: Copy>(values: &[T]) -> &[u8] {
    // SAFETY: the slice is valid for its size in bytes, and the types passed here have no padding.
    unsafe { std::slice::from_raw_parts(values.as_ptr().cast(), std::mem::size_of_val(values)) }
}

macro_rules! object {
    ($(#[$meta:meta])* $name:ident, |$id:ident| $delete:expr) => {
        $(#[$meta])*
        #[derive(Default)]
        pub struct $name(GLuint);

        impl Drop for $name {
            fn drop(&mut self) {
                if self.0 != 0 {
                    let $id = self.0;
                    unsafe { $delete }
                }
            }
        }
    };
}

object!(
    /// Owns a buffer of vertices, indices or uniforms.
    Buffer, |id| gl::DeleteBuffers(1, &id));
object!(VertexArray, |id| gl::DeleteVertexArrays(1, &id));
object!(Texture, |id| gl::DeleteTextures(1, &id));
object!(Sampler, |id| gl::DeleteSamplers(1, &id));
object!(Framebuffer, |id| gl::DeleteFramebuffers(1, &id));

impl Buffer {
    fn create() -> Buffer {
        let mut id = 0;
        unsafe { gl::CreateBuffers(1, &mut id) };
        Buffer(id)
    }

    /// A buffer whose contents are fixed.
    pub fn with_data(data: &[u8]) -> Buffer {
        let buffer = Buffer::create();
        unsafe { gl::NamedBufferStorage(buffer.0, data.len() as GLsizeiptr, data.as_ptr().cast(), 0) };
        buffer
    }

    /// A buffer of a fixed size, written with `write`.
    pub fn with_size(size: usize) -> Buffer {
        let buffer = Buffer::create();
        unsafe { gl::NamedBufferStorage(buffer.0, size as GLsizeiptr, std::ptr::null(), gl::DYNAMIC_STORAGE_BIT) };
        buffer
    }

    /// A buffer filled anew every frame with `stream`.
    pub fn streaming() -> Buffer {
        Buffer::create()
    }

    pub fn write(&self, data: &[u8]) {
        unsafe { gl::NamedBufferSubData(self.0, 0, data.len() as GLsizeiptr, data.as_ptr().cast()) };
    }

    pub fn stream(&self, data: &[u8]) {
        unsafe { gl::NamedBufferData(self.0, data.len() as GLsizeiptr, data.as_ptr().cast(), gl::STREAM_DRAW) };
    }

    pub fn bind_uniform_block(&self, index: GLuint) {
        unsafe { gl::BindBufferBase(gl::UNIFORM_BUFFER, index, self.0) };
    }
}

/// An attribute of a vertex: how many components of which type, and where in the vertex they start.
pub struct Attribute {
    pub size: GLint,
    pub kind: GLenum,
    pub offset: usize,
}

impl VertexArray {
    /// An array without attributes, for the shaders that make up their vertices.
    pub fn empty() -> VertexArray {
        let mut id = 0;
        unsafe { gl::CreateVertexArrays(1, &mut id) };
        VertexArray(id)
    }

    /// An array of vertices `stride` bytes apart; attributes of bytes are normalized.
    pub fn new(vertices: &Buffer, stride: usize, attributes: &[Attribute], indices: Option<&Buffer>) -> VertexArray {
        let array = VertexArray::empty();
        unsafe {
            gl::VertexArrayVertexBuffer(array.0, 0, vertices.0, 0, stride as GLsizei);
            if let Some(indices) = indices {
                gl::VertexArrayElementBuffer(array.0, indices.0);
            }
            for (index, attribute) in attributes.iter().enumerate() {
                let index = index as GLuint;
                let normalized = if attribute.kind == gl::UNSIGNED_BYTE { gl::TRUE } else { gl::FALSE };
                gl::EnableVertexArrayAttrib(array.0, index);
                gl::VertexArrayAttribFormat(array.0, index, attribute.size, attribute.kind, normalized, attribute.offset as GLuint);
                gl::VertexArrayAttribBinding(array.0, index, 0);
            }
        }
        array
    }

    pub fn bind(&self) {
        unsafe { gl::BindVertexArray(self.0) };
    }
}

impl Texture {
    pub fn new(target: GLenum) -> Texture {
        let mut id = 0;
        unsafe { gl::CreateTextures(target, 1, &mut id) };
        Texture(id)
    }

    pub fn storage(&self, levels: i32, format: GLenum, width: i32, height: i32) {
        unsafe { gl::TextureStorage2D(self.0, levels, format, width, height) };
    }

    pub fn storage_multisample(&self, samples: i32, format: GLenum, width: i32, height: i32) {
        unsafe { gl::TextureStorage2DMultisample(self.0, samples, format, width, height, gl::TRUE) };
    }

    /// Fills the first level of a texture of one byte a pixel.
    pub fn upload_r8(&self, width: i32, height: i32, pixels: &[u8]) {
        assert!(pixels.len() >= (width * height) as usize);
        unsafe {
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
            gl::TextureSubImage2D(self.0, 0, 0, 0, width, height, gl::RED, gl::UNSIGNED_BYTE, pixels.as_ptr().cast());
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 4);
        }
    }

    pub fn parameter(&self, name: GLenum, value: GLenum) {
        unsafe { gl::TextureParameteri(self.0, name, value as GLint) };
    }

    pub fn parameter_f32(&self, name: GLenum, value: f32) {
        unsafe { gl::TextureParameterf(self.0, name, value) };
    }

    pub fn set_filter(&self, min_filter: GLenum, mag_filter: GLenum, wrap: GLenum) {
        self.parameter(gl::TEXTURE_MIN_FILTER, min_filter);
        self.parameter(gl::TEXTURE_MAG_FILTER, mag_filter);
        self.parameter(gl::TEXTURE_WRAP_S, wrap);
        self.parameter(gl::TEXTURE_WRAP_T, wrap);
        self.parameter(gl::TEXTURE_WRAP_R, wrap);
    }

    pub fn generate_mipmap(&self) {
        unsafe { gl::GenerateTextureMipmap(self.0) };
    }

    pub fn bind(&self, unit: GLuint) {
        unsafe { gl::BindTextureUnit(unit, self.0) };
    }

    /// Binds a level for a compute shader to write; `layered` binds all the faces of a cube map.
    pub fn bind_image(&self, unit: GLuint, level: i32, layered: bool, format: GLenum) {
        unsafe { gl::BindImageTexture(unit, self.0, level, if layered { gl::TRUE } else { gl::FALSE }, 0, gl::WRITE_ONLY, format) };
    }
}

/// Leaves a texture unit without a texture.
pub fn unbind_texture(unit: GLuint) {
    unsafe { gl::BindTextureUnit(unit, 0) };
}

impl Sampler {
    pub fn new() -> Sampler {
        let mut id = 0;
        unsafe { gl::CreateSamplers(1, &mut id) };
        Sampler(id)
    }

    pub fn parameter(&self, name: GLenum, value: GLenum) {
        unsafe { gl::SamplerParameteri(self.0, name, value as GLint) };
    }

    pub fn bind(&self, unit: GLuint) {
        unsafe { gl::BindSampler(unit, self.0) };
    }
}

impl Framebuffer {
    pub fn new() -> Framebuffer {
        let mut id = 0;
        unsafe { gl::CreateFramebuffers(1, &mut id) };
        Framebuffer(id)
    }

    pub fn attach(&self, attachment: GLenum, texture: &Texture) {
        unsafe { gl::NamedFramebufferTexture(self.0, attachment, texture.0, 0) };
    }

    /// For a framebuffer with depth only.
    pub fn without_color(&self) {
        unsafe { gl::NamedFramebufferDrawBuffer(self.0, gl::NONE) };
    }

    pub fn check(&self, name: &str) -> Result<()> {
        if unsafe { gl::CheckNamedFramebufferStatus(self.0, gl::FRAMEBUFFER) } != gl::FRAMEBUFFER_COMPLETE {
            return Err(format!("The {name} framebuffer is not complete.").into());
        }
        Ok(())
    }

    pub fn bind(&self) {
        unsafe { gl::BindFramebuffer(gl::FRAMEBUFFER, self.0) };
    }

    pub fn clear_color(&self, color: [f32; 4]) {
        unsafe { gl::ClearNamedFramebufferfv(self.0, gl::COLOR, 0, color.as_ptr()) };
    }

    pub fn clear_depth(&self, depth: f32) {
        unsafe { gl::ClearNamedFramebufferfv(self.0, gl::DEPTH, 0, &depth) };
    }
}

/// Draws into the window again.
pub fn bind_window_framebuffer() {
    unsafe { gl::BindFramebuffer(gl::FRAMEBUFFER, 0) };
}

// Shader programs ----------------------------------------------------------------------------------

static GLSL_VERSION: AtomicU32 = AtomicU32::new(460);

/// The GLSL version the shaders are compiled for, from the context's OpenGL version (46 for 4.6).
pub fn set_glsl_version(gl_version: i32) {
    GLSL_VERSION.store(if gl_version >= 46 { 460 } else { 450 }, Ordering::Relaxed);
}

/// A stage of a program: its kind and the sources it is put together from.
pub struct Stage<'a> {
    pub kind: GLenum,
    pub sources: &'a [&'a str],
}

/// A value a uniform can be set to.
pub trait Uniform {
    fn apply(&self, program: GLuint, location: GLint);
}

impl Uniform for i32 {
    fn apply(&self, program: GLuint, location: GLint) {
        unsafe { gl::ProgramUniform1i(program, location, *self) };
    }
}

impl Uniform for f32 {
    fn apply(&self, program: GLuint, location: GLint) {
        unsafe { gl::ProgramUniform1f(program, location, *self) };
    }
}

impl Uniform for Vec2 {
    fn apply(&self, program: GLuint, location: GLint) {
        unsafe { gl::ProgramUniform2f(program, location, self.x, self.y) };
    }
}

impl Uniform for Vec3 {
    fn apply(&self, program: GLuint, location: GLint) {
        unsafe { gl::ProgramUniform3f(program, location, self.x, self.y, self.z) };
    }
}

impl Uniform for Vec4 {
    fn apply(&self, program: GLuint, location: GLint) {
        unsafe { gl::ProgramUniform4f(program, location, self.x, self.y, self.z, self.w) };
    }
}

impl Uniform for Mat4 {
    fn apply(&self, program: GLuint, location: GLint) {
        unsafe { gl::ProgramUniformMatrix4fv(program, location, 1, gl::FALSE, self.m.as_ptr()) };
    }
}

/// The squares of a board as a mask of 64 bits, in two halves.
impl Uniform for u64 {
    fn apply(&self, program: GLuint, location: GLint) {
        unsafe { gl::ProgramUniform2ui(program, location, *self as u32, (*self >> 32) as u32) };
    }
}

impl Uniform for &[Vec4] {
    fn apply(&self, program: GLuint, location: GLint) {
        if !self.is_empty() {
            unsafe { gl::ProgramUniform4fv(program, location, self.len() as GLsizei, self.as_ptr().cast()) };
        }
    }
}

/// A linked shader program.
#[derive(Default)]
pub struct Program {
    id: GLuint,
    locations: RefCell<HashMap<&'static str, GLint>>,
}

impl Program {
    pub fn new(name: &str, stages: &[Stage]) -> Result<Program> {
        let program = Program { id: unsafe { gl::CreateProgram() }, locations: RefCell::default() };
        let mut shaders = Vec::new();
        let mut failure = None;
        for stage in stages {
            let mut source = format!("#version {} core\n", GLSL_VERSION.load(Ordering::Relaxed));
            for (index, part) in stage.sources.iter().enumerate() {
                source += &format!("#line 1 {index}\n{part}\n");
            }
            let shader = unsafe { gl::CreateShader(stage.kind) };
            shaders.push(shader);
            let mut compiled = 0;
            unsafe {
                let text = source.as_ptr().cast::<GLchar>();
                let length = source.len() as GLint;
                gl::ShaderSource(shader, 1, &text, &length);
                gl::CompileShader(shader);
                gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut compiled);
            }
            if compiled == 0 {
                let log = info_log(|capacity, length, log| unsafe { gl::GetShaderInfoLog(shader, capacity, length, log) });
                failure = Some(format!("The {name} shader does not compile:\n{log}"));
                break;
            }
            unsafe { gl::AttachShader(program.id, shader) };
        }
        if failure.is_none() {
            let mut linked = 0;
            unsafe {
                gl::LinkProgram(program.id);
                gl::GetProgramiv(program.id, gl::LINK_STATUS, &mut linked);
            }
            if linked == 0 {
                let log = info_log(|capacity, length, log| unsafe { gl::GetProgramInfoLog(program.id, capacity, length, log) });
                failure = Some(format!("The {name} shaders do not link:\n{log}"));
            }
        }
        for shader in shaders {
            unsafe { gl::DeleteShader(shader) };
        }
        match failure {
            Some(message) => Err(message.into()),
            None => Ok(program),
        }
    }

    pub fn bind(&self) {
        unsafe { gl::UseProgram(self.id) };
    }

    pub fn set(&self, name: &'static str, value: impl Uniform) {
        value.apply(self.id, self.location(name));
    }

    fn location(&self, name: &'static str) -> GLint {
        *self.locations.borrow_mut().entry(name).or_insert_with(|| {
            let name = CString::new(name).expect("a uniform's name has no zero bytes");
            unsafe { gl::GetUniformLocation(self.id, name.as_ptr()) }
        })
    }
}

impl Drop for Program {
    fn drop(&mut self) {
        if self.id != 0 {
            unsafe { gl::DeleteProgram(self.id) };
        }
    }
}

fn info_log(read: impl Fn(GLsizei, *mut GLsizei, *mut GLchar)) -> String {
    let mut log = vec![0u8; 4096];
    let mut length = 0;
    read(log.len() as GLsizei, &mut length, log.as_mut_ptr().cast());
    log.truncate(length.max(0) as usize);
    String::from_utf8_lossy(&log).into_owned()
}

// State and drawing --------------------------------------------------------------------------------

pub fn enable(capability: GLenum) {
    unsafe { gl::Enable(capability) };
}

pub fn disable(capability: GLenum) {
    unsafe { gl::Disable(capability) };
}

pub fn viewport(width: i32, height: i32) {
    unsafe { gl::Viewport(0, 0, width, height) };
}

pub fn clear_window(red: f32, green: f32, blue: f32) {
    unsafe {
        gl::ClearColor(red, green, blue, 1.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);
    }
}

pub fn depth_func(function: GLenum) {
    unsafe { gl::DepthFunc(function) };
}

pub fn depth_mask(write: bool) {
    unsafe { gl::DepthMask(if write { gl::TRUE } else { gl::FALSE }) };
}

pub fn cull_face(mode: GLenum) {
    unsafe { gl::CullFace(mode) };
}

pub fn front_face(mode: GLenum) {
    unsafe { gl::FrontFace(mode) };
}

pub fn blend_func(source: GLenum, destination: GLenum) {
    unsafe { gl::BlendFunc(source, destination) };
}

pub fn blend_func_separate(source_rgb: GLenum, destination_rgb: GLenum, source_alpha: GLenum, destination_alpha: GLenum) {
    unsafe { gl::BlendFuncSeparate(source_rgb, destination_rgb, source_alpha, destination_alpha) };
}

pub fn polygon_offset(factor: f32, units: f32) {
    unsafe { gl::PolygonOffset(factor, units) };
}

pub fn draw_triangles(count: i32) {
    unsafe { gl::DrawArrays(gl::TRIANGLES, 0, count) };
}

/// Draws triangles from the bound array's indices, which are 32 bits wide.
pub fn draw_indexed_triangles(count: i32) {
    unsafe { gl::DrawElements(gl::TRIANGLES, count, gl::UNSIGNED_INT, std::ptr::null()) };
}

/// Runs the bound compute shader over an image in groups of 8 × 8, and waits for what it wrote.
pub fn dispatch_compute(width: i32, height: i32, layers: i32) {
    unsafe {
        gl::DispatchCompute(((width + 7) / 8) as GLuint, ((height + 7) / 8) as GLuint, layers as GLuint);
        gl::MemoryBarrier(gl::SHADER_IMAGE_ACCESS_BARRIER_BIT | gl::TEXTURE_FETCH_BARRIER_BIT);
    }
}

pub fn get_integer(name: GLenum) -> i32 {
    let mut value = 0;
    unsafe { gl::GetIntegerv(name, &mut value) };
    value
}

/// Anisotropic filtering, in the core since OpenGL 4.6 and an extension nearly everywhere before.
pub const TEXTURE_MAX_ANISOTROPY: GLenum = 0x84FE;

/// How far the driver can take anisotropic filtering; 1 where it cannot.
pub fn max_anisotropy() -> f32 {
    const MAX_TEXTURE_MAX_ANISOTROPY: GLenum = 0x84FF;
    let mut value = 1.0;
    unsafe { gl::GetFloatv(MAX_TEXTURE_MAX_ANISOTROPY, &mut value) };
    value.max(1.0)
}

/// Reports the driver's complaints on the standard error stream; for debug builds.
pub fn report_errors() {
    extern "system" fn message(_: GLenum, _: GLenum, _: GLuint, severity: GLenum, _: GLsizei, message: *const GLchar, _: *mut c_void) {
        if severity == gl::DEBUG_SEVERITY_HIGH || severity == gl::DEBUG_SEVERITY_MEDIUM {
            eprintln!("OpenGL: {}", unsafe { CStr::from_ptr(message) }.to_string_lossy());
        }
    }

    unsafe {
        gl::Enable(gl::DEBUG_OUTPUT);
        gl::Enable(gl::DEBUG_OUTPUT_SYNCHRONOUS);
        gl::DebugMessageCallback(Some(message), std::ptr::null());
    }
}
