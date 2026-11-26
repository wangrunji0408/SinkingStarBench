// Original static buffer declarations from 0x1401eeb10.
create((uint8_t*)0x14080a908ull,8192,32,8); // Primary_Fog_Of_War_Params
create((uint8_t*)0x14080a7f8ull,6144,48,8); // Secondary_Fog_Of_War_Params
create((uint8_t*)0x14080a6e8ull,8192,16,8); // Fog_Of_War_Clear_Params
create((uint8_t*)0x14080ab28ull,49152,48,8); // Fog_Volume_World
create((uint8_t*)0x14080aa18ull,49152,48,8); // Fog_Volume_Box_To_World
create((uint8_t*)0x14080ac38ull,49152,48,8); // Fog_Volume_Params
create((uint8_t*)0x14080ad48ull,480,480,4); // Fog_Voxelization_Params
create((uint8_t*)0x140a19b28ull,131072,16,48); // draw_argument_pool
create((uint8_t*)0x140ab9c48ull,7077888,864,8); // particle_params_pool
create((uint8_t*)0x1408d9a08ull,4194304,256,4); // uniform_pool_256
create((uint8_t*)0x1408b18e8ull,1048576,512,4); // uniform_pool_512
create((uint8_t*)0x1408897c8ull,8388608,4096,4); // uniform_pool_4096
create((uint8_t*)0x140815a08ull,33554432,4,19); // particle_vertex_block
create((uint8_t*)0x140815b48ull,301989888,4,3); // mesh_vertex_block
create((uint8_t*)0x14080bb08ull,1024,64,8); // spot_light_transforms_buffer
create((uint8_t*)0x14080b728ull,4092,12,16); // probe_average_radiance_buffer
create((uint8_t*)0x14080d3b0ull,144,48,8); // spectrum_buffer
create((uint8_t*)0x14080d4c0ull,144,48,8); // spectrum_buffer
create((uint8_t*)0x14080d5d0ull,144,48,8); // spectrum_buffer
create((uint8_t*)0x14080d6e0ull,144,48,8); // spectrum_buffer
create((uint8_t*)0x140809d08ull,5456,16,8); // light_probes_buffer
create((uint8_t*)0x140809bf8ull,5456,16,8); // light_probes_data_buffer
create((uint8_t*)0x140809ae8ull,16384,64,8); // point_lights_buffer
create((uint8_t*)0x1408099d8ull,8192,32,8); // point_light_shadows_buffer
create((uint8_t*)0x1408098c8ull,8192,32,8); // point_lights_buffer_cluster
create((uint8_t*)0x1408097b8ull,16384,64,8); // decal_parameters_buffer
create((uint8_t*)0x1408096a8ull,12288,48,8); // decal_ws_to_ds_buffer
create((uint8_t*)0x140809598ull,4096,32,8); // shadow_decals_buffer
create((uint8_t*)0x140809488ull,2048,16,8); // shadow_decals_cluster_buffer
create((uint8_t*)0x14080ce08ull,256,4,16); // histogram_gpu
create((uint8_t*)0x14080ccf8ull,32,32,20); // avg_luminance
create((uint8_t*)0x14080b348ull,16384,16,8); // ocean_discard_plane_buffer
create((uint8_t*)0x14080b238ull,4096,4,8); // group_buffer
create((uint8_t*)0x14080b128ull,6400,16,8); // bard_radius_point_buffer
create((uint8_t*)0x14080d2a0ull,4096,4,8); // scrape_layer_heap_index_buffer
create((uint8_t*)0x140b5a668ull,38400000,48,8); // entity_params_buffer
create((uint8_t*)0x140b5a790ull,38400000,48,8); // entity_matrices_buffer
create((uint8_t*)0x140b59fa8ull,393216,96,8); // particle_params_buffer
create((uint8_t*)0x140b5a0c8ull,480000,16,8); // outline_params_buffer
create((uint8_t*)0x140b5a548ull,24576,48,8); // rover_params_buffer
create((uint8_t*)0x140b5a428ull,1600,16,8); // voice_log_params_buffer
create((uint8_t*)0x140b5a1e8ull,2048,16,8); // ocean_buffer
create((uint8_t*)0x140b5a308ull,917504,112,8); // mirage_params_buffer
create((uint8_t*)0x140b59d68ull,6400000,16,8); // river_ripple_vertex_buffer
create((uint8_t*)0x140b59e88ull,1600000,16,8); // river_ripple_buffer
cpuMemory.emplace_back(38400000);*(void**)0x140b5a778ull=cpuMemory.back().data();
cpuMemory.emplace_back(38400000);*(void**)0x140b5a8a0ull=cpuMemory.back().data();
cpuMemory.emplace_back(393216);*(void**)0x140b5a0b8ull=cpuMemory.back().data();
cpuMemory.emplace_back(480000);*(void**)0x140b5a1d8ull=cpuMemory.back().data();
cpuMemory.emplace_back(24576);*(void**)0x140b5a658ull=cpuMemory.back().data();
cpuMemory.emplace_back(1600);*(void**)0x140b5a538ull=cpuMemory.back().data();
cpuMemory.emplace_back(2048);*(void**)0x140b5a2f8ull=cpuMemory.back().data();
cpuMemory.emplace_back(917504);*(void**)0x140b5a418ull=cpuMemory.back().data();
cpuMemory.emplace_back(6400000);*(void**)0x140b59e78ull=cpuMemory.back().data();
cpuMemory.emplace_back(1600000);*(void**)0x140b59f98ull=cpuMemory.back().data();
