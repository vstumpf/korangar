#![feature(unzip_option)]
#![feature(option_zip)]
#![feature(let_else)]
#![feature(adt_const_params)]
#![feature(arc_unwrap_or_clone)]

#[macro_use]
extern crate korangar_procedural;
extern crate derive_new;
extern crate vulkano;
extern crate vulkano_shaders;
extern crate vulkano_win;
extern crate winit;
extern crate num;
extern crate cgmath;
extern crate serde;
extern crate ron;
extern crate rusttype;
extern crate yazi;
extern crate image;
extern crate pathfinding;
extern crate chrono;
#[cfg(feature = "debug")]
#[macro_use]
extern crate lazy_static;
extern crate rusqlite;

#[cfg(feature = "debug")]
#[macro_use]
mod debug;
#[macro_use]
mod types;
mod traits;
mod input;
#[macro_use]
mod system;
mod graphics;
mod loaders;
mod interface;
mod network;
mod database;

use std::sync::Arc;
use std::cell::RefCell;
use std::rc::Rc;
use vulkano::device::Device;
use vulkano::instance::Instance;
#[cfg(feature = "debug")]
use vulkano::instance::debug::{ MessageSeverity, MessageType };
use vulkano::Version;
use vulkano::sync::{ GpuFuture, now };
use vulkano_win::VkSurfaceBuild;
use winit::event::{ Event, WindowEvent };
use winit::event_loop::{ ControlFlow, EventLoop };
use winit::window::WindowBuilder;
use database::Database;
use chrono::prelude::*;

#[cfg(feature = "debug")]
use crate::debug::*;
use crate::types::{ Entity, ChatMessage };
use crate::input::{ InputSystem, UserEvent };
use system::{ GameTimer, get_instance_extensions, get_layers, get_device_extensions };
use crate::loaders::{ FontLoader, GameFileLoader, MapLoader, ModelLoader, TextureLoader, SpriteLoader, ActionLoader };
use crate::graphics::{ Renderer, RenderSettings, Color };
use crate::graphics::camera::*;
use crate::interface::*;
use network::{ NetworkingSystem, NetworkEvent };

fn main() {

    #[cfg(feature = "debug")]
    let timer = Timer::new("create device");

    let instance = Instance::new(None, Version::V1_2, &get_instance_extensions(), get_layers()).expect("failed to create instance");

    #[cfg(feature = "debug")]
    let _debug_callback = vulkano::instance::debug::DebugCallback::new(&instance, MessageSeverity::all(), MessageType::all(), vulkan_message_callback).ok();

    #[cfg(feature = "debug")]
    print_debug!("created {}instance{}", MAGENTA, NONE);

    #[cfg(feature = "debug")]
    timer.stop();

    #[cfg(feature = "debug")]
    let timer = Timer::new("create window");

    let events_loop = EventLoop::new();
    let surface = WindowBuilder::new().with_title(String::from("korangar")).build_vk_surface(&events_loop, instance.clone()).unwrap();

    #[cfg(feature = "debug")]
    print_debug!("created {}window{}", MAGENTA, NONE);

    #[cfg(feature = "debug")]
    timer.stop();

    #[cfg(feature = "debug")]
    let timer = Timer::new("choose physical device");

    let desired_device_extensions = get_device_extensions();
    let (physical_device, queue_family) = choose_physical_device!(&instance, surface, &desired_device_extensions);
    let required_device_extensions = physical_device.required_extensions().union(&desired_device_extensions);

    #[cfg(feature = "debug")]
    timer.stop();

    #[cfg(feature = "debug")]
    let timer = Timer::new("create device");

    let (device, mut queues) = Device::new(physical_device, physical_device.supported_features(), &required_device_extensions, [(queue_family, 0.5)].iter().cloned()).expect("failed to create device");

    #[cfg(feature = "debug")]
    print_debug!("created {}vulkan device{}", MAGENTA, NONE);

    let queue = queues.next().unwrap();

    #[cfg(feature = "debug")]
    print_debug!("received {}queue{} from {}device{}", MAGENTA, NONE, MAGENTA, NONE);

    #[cfg(feature = "debug")]
    timer.stop();

    #[cfg(feature = "debug")]
    let timer = Timer::new("create resource managers");

    let mut game_file_loader = GameFileLoader::default();
    game_file_loader.add_archive("data3.grf".to_string());
    game_file_loader.add_archive("data.grf".to_string());
    game_file_loader.add_archive("rdata.grf".to_string()); 

    let game_file_loader = Rc::new(RefCell::new(game_file_loader));

    let font_loader = Rc::new(RefCell::new(FontLoader::new(device.clone(), queue.clone())));
    let mut model_loader = ModelLoader::new(game_file_loader.clone(), device.clone());
    let mut texture_loader = TextureLoader::new(game_file_loader.clone(), device.clone(), queue.clone());
    let mut map_loader = MapLoader::new(game_file_loader.clone(), device.clone());
    let mut sprite_loader = SpriteLoader::new(game_file_loader.clone(), device.clone(), queue.clone());
    let mut action_loader = ActionLoader::new(game_file_loader.clone());

    #[cfg(feature = "debug")]
    timer.stop();

    #[cfg(feature = "debug")]
    let timer = Timer::new("load resources");

    let mut map = Arc::new(map_loader.get(&mut model_loader, &mut texture_loader, "pay_dun00.rsw").expect("failed to load initial map"));

    // interesting: ma_zif07, ama_dun01

    #[cfg(feature = "debug")]
    timer.stop();

    #[cfg(feature = "debug")]
    let timer = Timer::new("create renderer");

    let mut renderer = Renderer::new(&physical_device, device.clone(), queue, surface.clone(), &mut texture_loader, font_loader.clone());

    #[cfg(feature = "debug")]
    timer.stop();

    #[cfg(feature = "debug")]
    let timer = Timer::new("initialize interface");

    let window_size = renderer.get_window_size().map(|c| c as f32);
    let mut interface = Interface::new(window_size);
    let mut input_system = InputSystem::new();
    let mut render_settings = RenderSettings::new();

    #[cfg(feature = "debug")]
    timer.stop();

    #[cfg(feature = "debug")]
    let timer = Timer::new("initialize timer");

    let mut game_timer = GameTimer::new();

    #[cfg(feature = "debug")]
    timer.stop();

    #[cfg(feature = "debug")]
    let timer = Timer::new("initialize camera");

    #[cfg(feature = "debug")]
    let mut debug_camera = DebugCamera::new();
    let mut player_camera = PlayerCamera::new();

    #[cfg(feature = "debug")]
    timer.stop();

    #[cfg(feature = "debug")]
    let timer = Timer::new("initialize networking");

    let mut networking_system = NetworkingSystem::new();

    match networking_system.log_in() { 
        Ok(character_selection_window) => interface.open_window(&character_selection_window),
        Err(message) => interface.open_window(&ErrorWindow::new(message)),
    }

    #[cfg(feature = "debug")]
    timer.stop();

    let mut texture_future = now(device.clone()).boxed();

    let mut entities = Arc::new(Vec::new());

    // move
    const TIME_FACTOR: f32 = 1000.0;
    let local: DateTime<Local> = Local::now();
    let mut day_timer = (local.hour() as f32 / TIME_FACTOR * 60.0 * 60.0) + (local.minute() as f32 / TIME_FACTOR * 60.0);
    //

    let chat_messages = Rc::new(RefCell::new(vec![ChatMessage::new("Welcome to Korangar!".to_string(), Color::rgb(220, 170, 220))]));

    let database = Database::new();

    texture_future.flush().unwrap();
    texture_future.cleanup_finished();

    events_loop.run(move |event, _, control_flow| {
        match event {

            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                *control_flow = ControlFlow::Exit;
            }

            Event::WindowEvent { event: WindowEvent::Resized(_), .. } => {
                let window_size = surface.window().inner_size();
                let window_size = Size::new(window_size.width as f32, window_size.height as f32);
                interface.update_window_size(window_size);
                renderer.invalidate_swapchain();
            }

            Event::WindowEvent { event: WindowEvent::Focused(focused), .. } => {
                if !focused {
                    input_system.reset();
                }
            }

            Event::WindowEvent { event: WindowEvent::CursorMoved { position, .. }, .. } => {
                input_system.update_mouse_position(position);
            }

            Event::WindowEvent { event: WindowEvent::MouseInput{ button, state, .. }, .. } => {
                input_system.update_mouse_buttons(button, state);
            }

            Event::WindowEvent { event: WindowEvent::MouseWheel{ delta, .. }, .. } => {
                input_system.update_mouse_wheel(delta);
            }

            Event::WindowEvent { event: WindowEvent::KeyboardInput{ input, .. }, .. } => {
                input_system.update_keyboard(input.scancode as usize, input.state);
            }

            Event::RedrawEventsCleared => {

                input_system.update_delta();

                let delta_time = game_timer.update();

                day_timer += delta_time as f32 / TIME_FACTOR;

                networking_system.keep_alive(delta_time, game_timer.get_client_tick());

                let network_events = networking_system.network_events();
                let (user_events, hovered_element) = input_system.user_events(&mut renderer, &mut interface, &render_settings);

                let mut texture_future = now(device.clone()).boxed();
                let mut entities_copy = std::borrow::Cow::Borrowed(&*entities);

                for event in network_events {
                    match event {

                        NetworkEvent::AddEntity(entity_id, job_id, position, movement_speed) => {
                            entities_copy.to_mut().push(Arc::new(Entity::new(&mut sprite_loader, &mut action_loader, &mut texture_future, &map, &database, entity_id, job_id, position, movement_speed)));
                        }

                        NetworkEvent::RemoveEntity(entity_id) => {
                            entities_copy.to_mut().retain(|entity| entity.entity_id != entity_id);
                        }

                        NetworkEvent::EntityMove(entity_id, position_from, position_to, starting_timestamp) => {
                            let entities = entities_copy.to_mut();
                            let entity_index = entities.iter().position(|entity| entity.entity_id == entity_id).expect("failed to find entity");
                            let mut entity = Clone::clone(&*entities[entity_index]);

                            entity.move_from_to(&map, position_from, position_to, starting_timestamp);
                            #[cfg(feature = "debug")]
                            entity.generate_steps_vertex_buffer(device.clone(), &map);

                            entities[entity_index] = Arc::new(entity);
                        }

                        NetworkEvent::PlayerMove(position_from, position_to, starting_timestamp) => {
                            let entities = entities_copy.to_mut();
                            let mut entity = Clone::clone(&*entities[0]);

                            entity.move_from_to(&map, position_from, position_to, starting_timestamp);
                            #[cfg(feature = "debug")]
                            entity.generate_steps_vertex_buffer(device.clone(), &map);

                            entities[0] = Arc::new(entity);
                        }

                        NetworkEvent::ChangeMap(map_name, player_position) => {
                            let entities = entities_copy.to_mut();

                            while entities.len() > 1 {
                                entities.pop(); 
                            }

                            map = Arc::new(map_loader.get(&mut model_loader, &mut texture_loader, &format!("{}.rsw", map_name)).unwrap());

                            let mut player = Arc::unwrap_or_clone(entities.remove(0));

                            player.set_position(&map, player_position);
                            player_camera.set_focus_point(player.position);

                            entities.push(Arc::new(player));

                            networking_system.map_loaded();
                        }

                        NetworkEvent::UpdataClientTick(client_tick) => {
                            game_timer.set_client_tick(client_tick);
                        }

                        NetworkEvent::ChatMessage(message) => {
                            chat_messages.borrow_mut().push(message);
                        }
                    }
                }

                for event in user_events {
                    match event {

                        UserEvent::Exit => *control_flow = ControlFlow::Exit,

                        UserEvent::LogOut => networking_system.log_out().unwrap(),

                        UserEvent::CameraZoom(factor) => player_camera.soft_zoom(factor),

                        UserEvent::CameraRotate(factor) => player_camera.soft_rotate(factor),

                        UserEvent::ToggleFrameLimit => {
                            render_settings.toggle_frame_limit();
                            renderer.set_frame_limit(render_settings.frame_limit);
                            interface.schedule_rerender();
                            interface.open_window(&debug_camera.light_variables);
                        },

                        UserEvent::OpenMenuWindow => interface.open_window(&MenuWindow::default()),

                        UserEvent::OpenGraphicsSettingsWindow => interface.open_window(&GraphicsSettingsWindow::default()),

                        UserEvent::OpenAudioSettingsWindow => interface.open_window(&AudioSettingsWindow::default()),

                        UserEvent::ReloadTheme => interface.reload_theme(),

                        UserEvent::SaveTheme => interface.save_theme(),

                        UserEvent::SelectCharacter(character_slot) => {
                            match networking_system.select_character(character_slot, &chat_messages) {

                                Ok((map_name, player_position, character_id, job_id, movement_speed, client_tick)) => {

                                    interface.close_window_with_class("character_selection"); // FIND A NICER WAY TO GET THE CLASS NAME
                                    interface.open_window(&PrototypeChatWindow::new(chat_messages.clone()));

                                    map = Arc::new(map_loader.get(&mut model_loader, &mut texture_loader, &format!("{}.rsw", map_name)).unwrap());

                                    let player = Entity::new(&mut sprite_loader, &mut action_loader, &mut texture_future, &map, &database, character_id, job_id, player_position, movement_speed);

                                    player_camera.set_focus_point(player.position);
                                    entities_copy.to_mut().push(Arc::new(player));

                                    networking_system.map_loaded();
                                    game_timer.set_client_tick(client_tick);
                                }

                                Err(message) => interface.open_window(&ErrorWindow::new(message)),
                            }
                        },

                        UserEvent::CreateCharacter(character_slot) => {
                            if let Err(message) = networking_system.crate_character(character_slot) {
                                interface.open_window(&ErrorWindow::new(message));
                            } 
                        },

                        UserEvent::DeleteCharacter(character_id) => {
                            if let Err(message) = networking_system.delete_character(character_id) {
                                interface.open_window(&ErrorWindow::new(message));
                            }
                        },

                        UserEvent::RequestSwitchCharacterSlot(origin_slot) => networking_system.request_switch_character_slot(origin_slot),

                        UserEvent::CancelSwitchCharacterSlot => networking_system.cancel_switch_character_slot(),

                        UserEvent::SwitchCharacterSlot(destination_slot) => {
                            if let Err(message) = networking_system.switch_character_slot(destination_slot) {
                                interface.open_window(&ErrorWindow::new(message));
                            }
                        },

                        UserEvent::RequestPlayerMove(destination) => networking_system.request_player_move(destination),

                        UserEvent::RequestWarpToMap(map_name, position) => networking_system.request_warp_to_map(map_name, position),

                        #[cfg(feature = "debug")]
                        UserEvent::OpenRenderSettingsWindow => interface.open_window(&RenderSettingsWindow::default()),

                        #[cfg(feature = "debug")]
                        UserEvent::OpenMapDataWindow => interface.open_window(map.to_prototype_window()),

                        #[cfg(feature = "debug")]
                        UserEvent::OpenMapsWindow => interface.open_window(&MapsWindow::new()),

                        #[cfg(feature = "debug")]
                        UserEvent::OpenTimeWindow => interface.open_window(&TimeWindow::new()),

                        #[cfg(feature = "debug")]
                        UserEvent::SetDawn => day_timer = 0.0,

                        #[cfg(feature = "debug")]
                        UserEvent::SetNoon => day_timer = std::f32::consts::FRAC_PI_2,

                        #[cfg(feature = "debug")]
                        UserEvent::SetDusk => day_timer = std::f32::consts::PI,

                        #[cfg(feature = "debug")]
                        UserEvent::SetMidnight => day_timer = -std::f32::consts::FRAC_PI_2,

                        #[cfg(feature = "debug")]
                        UserEvent::OpenThemeViewerWindow => interface.open_theme_viewer_window(),

                        #[cfg(feature = "debug")]
                        UserEvent::OpenProfilerWindow => interface.open_window(&ProfilerWindow::default()),

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleUseDebugCamera => render_settings.toggle_use_debug_camera(),

                        #[cfg(feature = "debug")]
                        UserEvent::CameraLookAround(offset) => debug_camera.look_around(offset),

                        #[cfg(feature = "debug")]
                        UserEvent::CameraMoveForward => debug_camera.move_forward(delta_time as f32),

                        #[cfg(feature = "debug")]
                        UserEvent::CameraMoveBackward => debug_camera.move_backward(delta_time as f32),

                        #[cfg(feature = "debug")]
                        UserEvent::CameraMoveLeft => debug_camera.move_left(delta_time as f32),

                        #[cfg(feature = "debug")]
                        UserEvent::CameraMoveRight => debug_camera.move_right(delta_time as f32),

                        #[cfg(feature = "debug")]
                        UserEvent::CameraMoveUp => debug_camera.move_up(delta_time as f32),

                        #[cfg(feature = "debug")]
                        UserEvent::CameraAccelerate => debug_camera.accelerate(),

                        #[cfg(feature = "debug")]
                        UserEvent::CameraDecelerate => debug_camera.decelerate(),

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleShowFramesPerSecond => render_settings.toggle_show_frames_per_second(),

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleShowWireframe => {
                            render_settings.toggle_show_wireframe();
                            renderer.invalidate_swapchain();
                        },

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleShowMap => render_settings.toggle_show_map(),

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleShowObjects => render_settings.toggle_show_objects(),

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleShowEntities => render_settings.toggle_show_entities(),

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleShowWater => render_settings.toggle_show_water(),

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleShowAmbientLight => render_settings.toggle_show_ambient_light(),

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleShowDirectionalLight => render_settings.toggle_show_directional_light(),

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleShowPointLights => render_settings.toggle_show_point_lights(),

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleShowParticleLights => render_settings.toggle_show_particle_lights(),

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleShowDirectionalShadows => render_settings.toggle_show_directional_shadows(),

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleShowObjectMarkers => render_settings.toggle_show_object_markers(),

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleShowLightMarkers => render_settings.toggle_show_light_markers(),

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleShowSoundMarkers => render_settings.toggle_show_sound_markers(),

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleShowEffectMarkers => render_settings.toggle_show_effect_markers(),

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleShowParticleMarkers => render_settings.toggle_show_particle_markers(),

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleShowEntityMarkers => render_settings.toggle_show_entity_markers(),

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleShowMapTiles => render_settings.toggle_show_map_tiles(),

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleShowPathing => render_settings.toggle_show_pathing(),

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleShowDiffuseBuffer => render_settings.toggle_show_diffuse_buffer(),

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleShowNormalBuffer => render_settings.toggle_show_normal_buffer(),

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleShowWaterBuffer => render_settings.toggle_show_water_buffer(),

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleShowDepthBuffer => render_settings.toggle_show_depth_buffer(),

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleShowShadowBuffer => render_settings.toggle_show_shadow_buffer(),

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleShowPickerBuffer => render_settings.toggle_show_picker_buffer(),

                        #[cfg(feature = "debug")]
                        UserEvent::ToggleShowFontAtlas => render_settings.toggle_show_font_atlas(),
                    }
                }

                let texture_fence = texture_future
                    .queue()
                    .map(|_| texture_future.then_signal_fence_and_flush().unwrap().into())
                    .unwrap_or_default();

                for entity_index in 0..entities_copy.len() {
                    if entities_copy[entity_index].has_updates() {
                        let entities = entities_copy.to_mut();
                        let mut entity = Clone::clone(&*entities[entity_index]);
                        entity.update(&map, delta_time as f32, game_timer.get_client_tick());
                        entities[entity_index] = Arc::new(entity);
                    }
                }

                if let std::borrow::Cow::Owned(new_entities) = entities_copy {
                    entities = Arc::new(new_entities);
                }

                if !entities.is_empty() {
                    player_camera.set_focus_point(entities[0].position);
                }

                player_camera.update(delta_time);

                //map.update(delta_time as f32);

                renderer.update(delta_time as f32);

                let renderer_interface = interface.update();

                networking_system.changes_applied();

                renderer.start_frame(&surface, &render_settings, renderer_interface);

                if let Some(fence) = texture_fence {
                    fence.wait(None).unwrap();
                }

                #[cfg(feature = "debug")]
                let current_camera: &mut dyn Camera = match render_settings.use_debug_camera {
                    true => &mut debug_camera,
                    false => &mut player_camera,
                };

                #[cfg(not(feature = "debug"))]
                let current_camera = &mut player_camera;

                current_camera.generate_view_projection(renderer.get_window_size(), day_timer);

                // RENDER TO THE PICKER BUFFER INSTEAD?
                #[cfg(feature = "debug")]
                let hovered_marker = map.hovered_marker(&mut renderer, current_camera, &render_settings, &entities, input_system.mouse_position());

                // RENDER TO THE PICKER BUFFER INSTEAD?
                #[cfg(feature = "debug")]
                if let Some(marker) = hovered_marker {
                    if input_system.unused_left_click() {
                        let prototype_window = map.resolve_marker(&entities, marker);
                        interface.open_window(prototype_window);
                        input_system.set_interface_clicked();
                    }
                }

                map.render_picker(&mut renderer, current_camera);

                renderer.geometry_pass();

                //current_camera.render_scene(/*&mut renderer,*/ Arc::clone(&map), Arc::clone(&entities), &render_settings, game_timer.get_client_tick());

                //thread || {
                //    directional_camera.render_scene(&mut shadow_renderer, &map, &entities, &render_settings, game_timer.get_client_tick());
                //}

                map.render_geomitry(&mut renderer, current_camera, &render_settings, game_timer.get_client_tick());

                //if renderer_interface {
                //    renderer.render_text_new("test", vector2!(0.0), vector2!(300.0), Color::rgb(100, 255, 100), 10.0);
                //}

                #[cfg(feature = "debug")]
                if let Some(marker) = hovered_marker {
                    map.render_marker_box(&mut renderer, current_camera, marker);
                }

                if render_settings.show_entities {
                    entities.iter().for_each(|entity| entity.render(&mut renderer, current_camera));
                }

                if render_settings.show_water {
                    map.render_water(&mut renderer, current_camera);
                }

                #[cfg(feature = "debug")]
                if render_settings.show_pathing {
                    entities.iter().for_each(|entity| entity.render_pathing(&mut renderer, current_camera));
                }

                let state_provider = StateProvider::new(&render_settings);
                interface.render(&mut renderer, &state_provider, hovered_element);

                renderer.lighting_pass();
                let font_atlas_future = font_loader.borrow_mut().flush();

                #[cfg(feature = "debug")]
                match render_settings.show_buffers() {
                    true => renderer.render_buffers(&render_settings, font_loader.borrow().get_font_atlas()),
                    false => map.render_lights(&mut renderer, current_camera, &render_settings, day_timer),
                }

                #[cfg(not(feature = "debug"))]
                map.render_lights(&mut renderer, current_camera, &render_settings);

                #[cfg(feature = "debug")]
                map.render_markers(&mut renderer, current_camera, &render_settings, &entities, hovered_marker);

                #[cfg(feature = "debug")]

                renderer.render_interface(&render_settings);

                if render_settings.show_frames_per_second {
                    interface.render_frames_per_second(&mut renderer, game_timer.last_frames_per_second());
                }

                renderer.stop_frame(font_atlas_future);
            }

            _ignored => ()
        }
    });
}
