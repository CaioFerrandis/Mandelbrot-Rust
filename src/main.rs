use gl::*;
use glfw::*;
use std::sync::mpsc::Receiver;
use std::ffi::{CString, CStr};
use std::ptr;
use std::str;
use std::mem;
use std::time::{Instant, Duration};
use std::os::raw::c_void;
use gl::types::*;
use humantime::format_duration;

const vertexShaderSource: &str = r#"
    #version 330 core

    layout(location = 0) in vec2 in_position; // Define input position attribute

    out vec2 position; // Define output position varying variable

    void main() {
        gl_Position = vec4(in_position.xy, 0.0, 1.0);
        position = in_position; // Pass input position to the fragment shader
    }
"#;

const fragmentShaderSource: &str = r#"
    #version 330 core
    in vec2 position;
    out vec4 FragColor;

    uniform float time;
    uniform float zoom;

    uniform int substeps;

    uniform vec2 offset;

    vec4 mandelbrot(){
        vec2 z = vec2(0.);
        vec2 c = position;
        c *= zoom;
        c += offset;

        for (int i = 0; i <= substeps; i++){
            z = vec2(z.x * z.x - z.y * z.y, 2.0 * z.x * z.y) + c;
            if (length(z) > 4.){
                return vec4(float(i/substeps), float(i/substeps), float(i/substeps), 1.);
            }
        }
        return vec4(1.);
    }

    void main() {
        vec4 color = mandelbrot();

        FragColor = color;
    }
"#;

fn main() {
    let mut glfw = glfw::init(glfw::fail_on_errors).unwrap();

    let (mut window, events) = glfw.create_window(600, 600, "🤓", glfw::WindowMode::Windowed)
        .expect("Failed to create GLFW window.");

    window.set_key_polling(true);
    window.make_current();

    load_with(|s| window.get_proc_address(s) as * const _);

    let (shaderProgram, VAO) = unsafe {
        // build and compile our shader program
        // ------------------------------------
        // vertex shader
        let vertexShader = CreateShader(gl::VERTEX_SHADER);
        let c_str_vert = CString::new(vertexShaderSource.as_bytes()).unwrap();
        ShaderSource(vertexShader, 1, &c_str_vert.as_ptr(), ptr::null());
        CompileShader(vertexShader);

        // check for shader compile errors
        let mut success = FALSE as GLint;
        let mut infoLog = Vec::with_capacity(512);
        infoLog.set_len(512 - 1); // subtract 1 to skip the trailing null character
        GetShaderiv(vertexShader, COMPILE_STATUS, &mut success);
        if success != TRUE as GLint {
            GetShaderInfoLog(vertexShader, 512, ptr::null_mut(), infoLog.as_mut_ptr() as *mut GLchar);
            println!("ERROR::SHADER::VERTEX::COMPILATION_FAILED\n{}", str::from_utf8(&infoLog).unwrap());
        }

        // fragment shader
        let fragmentShader = gl::CreateShader(gl::FRAGMENT_SHADER);
        let c_str_frag = CString::new(fragmentShaderSource.as_bytes()).unwrap();
        ShaderSource(fragmentShader, 1, &c_str_frag.as_ptr(), ptr::null());
        CompileShader(fragmentShader);
        // check for shader compile errors
        GetShaderiv(fragmentShader, gl::COMPILE_STATUS, &mut success);
        if success != gl::TRUE as GLint {
            GetShaderInfoLog(fragmentShader, 512, ptr::null_mut(), infoLog.as_mut_ptr() as *mut GLchar);
            println!("ERROR::SHADER::FRAGMENT::COMPILATION_FAILED\n{}", str::from_utf8(&infoLog).unwrap());
        }

        // link shaders
        let shaderProgram = gl::CreateProgram();
        AttachShader(shaderProgram, vertexShader);
        AttachShader(shaderProgram, fragmentShader);
        LinkProgram(shaderProgram);
        // check for linking errors
        GetProgramiv(shaderProgram, gl::LINK_STATUS, &mut success);
        if success != gl::TRUE as GLint {
            GetProgramInfoLog(shaderProgram, 512, ptr::null_mut(), infoLog.as_mut_ptr() as *mut GLchar);
            println!("ERROR::SHADER::PROGRAM::COMPILATION_FAILED\n{}", str::from_utf8(&infoLog).unwrap());
        }
        DeleteShader(vertexShader);
        DeleteShader(fragmentShader);

        // set up vertex data (and buffer(s)) and configure vertex attributes
        // ------------------------------------------------------------------
        // HINT: type annotation is crucial since default for float literals is f64
        let vertices: [f32; 18] = [
            -1., -1., 0.0, // left1
             1., -1., 0.0, // right1
             -1.0,  1., 0.0,  // top1
             -1., 1., 0.0, // left2
             1., -1., 0.0, // right2
             1.0,  1., 0.0  // top2
        ];
        let (mut VBO, mut VAO) = (0, 0);
        gl::GenVertexArrays(1, &mut VAO);
        gl::GenBuffers(1, &mut VBO);
        // bind the Vertex Array Object first, then bind and set vertex buffer(s), and then configure vertex attributes(s).
        gl::BindVertexArray(VAO);

        gl::BindBuffer(gl::ARRAY_BUFFER, VBO);
        gl::BufferData(gl::ARRAY_BUFFER,
                       (vertices.len() * mem::size_of::<GLfloat>()) as GLsizeiptr,
                       &vertices[0] as *const f32 as *const c_void,
                       gl::STATIC_DRAW);

        gl::VertexAttribPointer(0, 3, gl::FLOAT, gl::FALSE, 3 * mem::size_of::<GLfloat>() as GLsizei, ptr::null());
        gl::EnableVertexAttribArray(0);

        // note that this is allowed, the call to gl::VertexAttribPointer registered VBO as the vertex attribute's bound vertex buffer object so afterwards we can safely unbind
        gl::BindBuffer(gl::ARRAY_BUFFER, 0);

        // You can unbind the VAO afterwards so other VAO calls won't accidentally modify this VAO, but this rarely happens. Modifying other
        // VAOs requires a call to glBindVertexArray anyways so we generally don't unbind VAOs (nor VBOs) when it's not directly necessary.
        gl::BindVertexArray(0);

        // uncomment this call to draw in wireframe polygons.
        //PolygonMode(gl::FRONT_AND_BACK, gl::LINE);

        (shaderProgram, VAO)
    };

    unsafe{UseProgram(shaderProgram);}

    let mut last_frame = Instant::now();
    let mut elapsed_time = Duration::new(0, 0);

    let mut zoom:f32 = 1.;
    let mut substeps:i32 = 1000;
    let mut offsetx:f32 = 0.;
    let mut offsety:f32 = 0.;

    while !window.should_close() {
        let now = Instant::now();
        let delta_time = now - last_frame;
        last_frame = now;

        // Update elapsed time
        elapsed_time += delta_time;

        glfw.poll_events();

        if (window.get_key(Key::I) == Action::Press){
            zoom /= 1.01;
        }
        if (window.get_key(Key::K) == Action::Press){
            zoom *= 1.01;
        }
        if (window.get_key(Key::W) == Action::Press){
            offsety += zoom/150.;
        }
        if (window.get_key(Key::S) == Action::Press){
            offsety -= zoom/150.;
        }
        if (window.get_key(Key::D) == Action::Press){
            offsetx += zoom/150.;
        }
        if (window.get_key(Key::A) == Action::Press){
            offsetx -= zoom/150.;
        }
        if (window.get_key(Key::Backspace) == Action::Press){
            offsetx = 0.;
            offsety = 0.;
            zoom = 1.;
        }
        if (window.get_key(Key::Up) == Action::Press){
            substeps += 1;
        }
        if (window.get_key(Key::Down) == Action::Press && substeps > 0){
            substeps -= 1;
        }
        println!("{substeps}");
        for (_, event) in glfw::flush_messages(&events) {
            handle_window_event(&mut window, event);
        }

        unsafe {
            ClearColor(0., 0., 0., 0.);
            Clear(COLOR_BUFFER_BIT | DEPTH_BUFFER_BIT);

            Uniform1f(
                GetUniformLocation(shaderProgram, CString::new("time").expect("aaaaa demonio").as_ptr()),
                elapsed_time.as_secs_f32()
            );

            Uniform1f(
                GetUniformLocation(shaderProgram, CString::new("zoom").expect("aaaaa demonio").as_ptr()),
                zoom
            );

            Uniform1i(
                GetUniformLocation(shaderProgram, CString::new("substeps").expect("aaaaa demonio").as_ptr()),
                substeps
            );

            Uniform2f(
                GetUniformLocation(shaderProgram, CString::new("offset").expect("aaaaa demonio").as_ptr()),
                offsetx,
                offsety
            );

            BindVertexArray(VAO);
            DrawArrays(TRIANGLES, 0, 6);
        }
        window.swap_buffers();
    }
}

fn handle_window_event(window: &mut glfw::Window, event: glfw::WindowEvent) {
    match event {
        glfw::WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
            window.set_should_close(true)
        }
        _ => {}
    }
}