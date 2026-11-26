#pragma once
#include "original_gpu_buffers.h"
#include "original_decoded_assets.h"
#include "original_render_resources.h"
#include "original_frame_debug.h"
#include "frame_slot.h"
struct OriginalFrame {
 const uint32_t width=renderWidth(),height=renderHeight();
 const uint32_t pitch=(width*4+255)&~255u; // D3D12_TEXTURE_DATA_PITCH_ALIGNMENT
 FrameSlot slot{size_t(width)*4,height};
 Host&host;OriginalRuntime&r;Bytes&jai;OriginalWorld&world;std::unique_ptr<OriginalGPUBuffers> buffers;std::unique_ptr<OriginalDecodedAssets> decoded;ID3D12Resource*readback=nullptr;bool submitted=false;uint64_t frameIndex=0;
 CommandBatch commands;Bytes context=Bytes(8848),frame=Bytes(256),pool=Bytes(40),temporary=Bytes(64*1024*1024),surface=Bytes(256),gpuAllocator=Bytes(40),linearUpload=Bytes(48),constantUpload=Bytes(48),outputTexture=Bytes(0x168),outputWindow=Bytes(160);
 OriginalFrame(Host&host,OriginalRuntime&r,Bytes&jai,OriginalWorld&world,bool immediately=true):host(host),r(r),jai(jai),world(world),commands(host){
  D3D12_FEATURE_DATA_D3D12_OPTIONS1 options1={};check(host.d->CheckFeatureSupport(D3D12_FEATURE_D3D12_OPTIONS1,&options1,sizeof options1),"Query wave size");if(!options1.WaveLaneCountMin)throw std::runtime_error("Device reports zero wave lanes");*(uint64_t*)0x140b6cb50=options1.WaveLaneCountMin;
  ID3D12Device1*device1=nullptr;check(host.d->QueryInterface(host.iid<ID3D12Device1>(),(void**)&device1),"Query original device1");host.keep(device1);*(void**)0x140884f60=device1;*(void**)context.data()=host.d;*(void**)(context.data()+0x18)=gpuAllocator.data();*(void**)(context.data()+8)=linearUpload.data();*(void**)(context.data()+16)=constantUpload.data();*(uint32_t*)(linearUpload.data()+40)=32*1024*1024;*(uint32_t*)(constantUpload.data()+40)=32*1024*1024;*(void**)(context.data()+0x20)=commands.list;*(void**)(context.data()+0x28)=commands.allocator;*(void**)(context.data()+0x30)=commands.list;*(void**)(context.data()+0x38)=commands.allocator;*(void**)(jai.data()+0x80)=context.data();*(void**)0x1408072e0=context.data();
  for(unsigned i=0;i<3;i++){D3D12_INDIRECT_ARGUMENT_DESC argument={};argument.Type=i==0?D3D12_INDIRECT_ARGUMENT_TYPE_DRAW:i==1?D3D12_INDIRECT_ARGUMENT_TYPE_DRAW_INDEXED:D3D12_INDIRECT_ARGUMENT_TYPE_DISPATCH;D3D12_COMMAND_SIGNATURE_DESC desc={};desc.ByteStride=i==0?16:i==1?20:12;desc.NumArgumentDescs=1;desc.pArgumentDescs=&argument;ID3D12CommandSignature*signature=nullptr;check(host.d->CreateCommandSignature(&desc,nullptr,host.iid<ID3D12CommandSignature>(),(void**)&signature),"Create original indirect command signature");host.keep(signature);*(void**)(context.data()+0x40+i*8)=signature;}
  *(uint32_t*)(surface.data()+0x38)=width;*(uint32_t*)(surface.data()+0x3c)=height;*(void**)0x140804c80=surface.data();
  *(uint32_t*)pool.data()=16;auto fields=(uintptr_t*)(pool.data()+8);fields[0]=fields[1]=(uintptr_t)temporary.data();fields[2]=fields[3]=(uintptr_t)(temporary.data()+temporary.size());*(uint64_t*)(frame.data()+0xf8)=1;
  D3D12_RECT scissor={0,0,16384,16384};commands.list->RSSetScissorRects(1,&scissor);commands.list->IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
  commands.list->SetDescriptorHeaps(1,(ID3D12DescriptorHeap**)0x1410d6e88);
  buffers=std::make_unique<OriginalGPUBuffers>(host,r,commands.list);
  decoded=std::make_unique<OriginalDecodedAssets>(r,"decoded-textures");
  if(getenv("SINKING_RENDER_DEBUG")){traceOriginalView(r);traceOriginalDraw(r);traceOriginalGeometry(r);}createOriginalRenderResources(r);
  if(getenv("SINKING_TRACE_BEAM"))for(auto &entry:Catalog::active->assets)if(entry.first=="data-pc/beam--100352.shader"&&entry.second.shader){auto sh=entry.second.shader.get();for(uintptr_t a=0x140b6c000;a<0x140b6e000;a+=8)if(*(void**)a==&sh->handle||*(void**)a==sh->handle)fprintf(stderr,"Beam global %llx\n",(unsigned long long)a);}

  *(float*)0x140b6c3c8=height;*(float*)0x140b6c3cc=width;*(float*)0x140b6c3d0=0;*(float*)0x140b6c3d4=0;
  auto ot=outputTexture.data();*(uint64_t*)ot=strlen("offscreen_backbuffer");*(const char**)(ot+8)="offscreen_backbuffer";*(uint32_t*)(ot+0x38)=width;*(uint32_t*)(ot+0x3c)=height;*(uint32_t*)(ot+0x40)=1;*(uint16_t*)(ot+0x44)=1;*(uint16_t*)(ot+0x46)=1;*(uint16_t*)(ot+0x48)=4;*(uint16_t*)(ot+0x4a)=0x21;ot[0x59]=1;ot[0x97]=0xff;uint8_t created=0;r.image.function<OriginalTextureCreate>(0x140163a20)((void*)0x140807260,ot,nullptr,nullptr,&created);*(void**)(outputWindow.data()+0x28)=ot;*(void**)0x140b6cfe8=outputWindow.data();
  using Component=void (__attribute__((ms_abi)) *)(uint8_t,void*);r.image.function<Component>(0x1400aab20)(0,(void*)0x140b6da00);auto initializeView=[&](uintptr_t at){using Append=void (__attribute__((ms_abi)) *)(void*,void*,void*);r.image.function<Append>(0x1401dd5b0)((void*)0x140807260,(void*)0x140b6c170,(void*)at);r.image.function<Component>(0x1400aab20)(1,(void*)(at+0xea8));r.image.function<Component>(0x1400aab20)(1,(void*)(at+0x1620));};
#include "original_view_components.inc"

  readback=host.buffer(size_t(pitch)*height,D3D12_HEAP_TYPE_READBACK);
  // Build gradient, opacity, emissive and scale lookup textures from each
  // original Particle_Source graph (the particle branch of level initialization).
  auto prepareParticles=[&](uint8_t* source){
   if(!*(uint64_t*)(source+0xc18))return;
   uint8_t original[14];memcpy(original,(void*)0x14043c0c3,14);
   r.image.redirect(0x14043c0c3,(void*)originalLayerEnd);
   originalSetupSection(jai.data(),source,0x14043a712);
   r.image.patch(0x14043c0c3,original,14);
  };
  prepareParticles(world.effects.library.data());
  prepareParticles(world.storage.data());
  if(immediately)draw();
 }
 void draw(){
  using Clock=std::chrono::steady_clock;const bool profile=getenv("SINKING_FRAME_PROFILE")!=nullptr;
  Clock::time_point begin,recorded,completed;if(profile)begin=Clock::now();


  if(submitted){commands.reset();buffers->releaseUploads();
   // draw() waited for the previous fence before returning. The EXE allocates
   // transient descriptors via heap+0x68[frameSlot] (0x14011ade0); permanent
   // descriptors and the start of each frame range are separate.
   for(uintptr_t heap:{0x1410d6d38ull,0x1410d6c90ull,0x1410d6de0ull,0x1410d6e88ull})
    memset((void*)(heap+0x68),0,16);
  }
  // The original frame setup (0x140276618) rewinds both the write and
  // upload cursors. Otherwise successive scenes exhaust the 512/100-entry
  // buffers checked by draw.jai even after the previous GPU fence completes.
  *(uint32_t*)0x140b5a660=*(uint32_t*)0x140b5a664=0;
  *(uint32_t*)0x140b5a540=*(uint32_t*)0x140b5a544=0;
  *(uint32_t*)0x140b5a300=0;*(uint32_t*)0x140b5a304=0; // New static scene submission; ocean CPU/upload cursors.
  *(uint32_t*)0x140b5a420=0;*(uint32_t*)0x140b5a424=0; // Per-frame mirage parameter write/upload cursors (0x140154d40 / 0x1401ae920).
  D3D12_RECT scissor={0,0,16384,16384};commands.list->RSSetScissorRects(1,&scissor);commands.list->IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);commands.list->SetDescriptorHeaps(1,(ID3D12DescriptorHeap**)0x1410d6e88);
  auto fields=(uintptr_t*)(pool.data()+8);fields[0]=fields[1]=(uintptr_t)temporary.data();fields[2]=fields[3]=(uintptr_t)(temporary.data()+temporary.size());memset(frame.data(),0,frame.size());*(uint64_t*)(frame.data()+0xf8)=1;
  uint64_t selected[2]={};using Selected=void (__attribute__((ms_abi)) *)(void*,void*,uint8_t,void*);r.image.function<Selected>(0x14031d8c0)(jai.data(),world.storage.data(),1,selected);fprintf(stderr,"Active family=%u movable=%llu selected=%llu\n",world.activeFamily,*(unsigned long long*)(world.storage.data()+0x920),selected[0]);
  auto ot=outputTexture.data();
  // Original immediate submission mode. Only a new host needs exposure prewarming.
  *(uint8_t*)0x140804a38=2;*(float*)0x140806110=1.0f/60.0f;
  world.updateCharmDisplay();
  const bool movie=getenv("SINKING_MOVIE");
  if(!movie)world.effects.resetParticles(r,jai,world.storage);
  const float effectStep=world.effects.frameStep(movie);
  using Init=void (__attribute__((ms_abi)) *)(void*,void*);fprintf(stderr,"Original frame context initialization\n");r.image.function<Init>(0x14011d9c0)(frame.data(),pool.data());fprintf(stderr,"Original scene preparation\n");world.selectCamera(r,jai);world.selectRooms(r,jai);world.selectSkies(r,jai);if(DeferredPipelines::enabled)DeferredPipelines::ensure(*(void***)0x140b6c650);r.image.function<Init>(0x14025a9a0)(frame.data(),world.storage.data());fprintf(stderr,"Original complete scene render matrixCount=%u paramCount=%u buckets=%llu camera=%g,%g,%g\n",*(unsigned*)0x140b5a780,*(unsigned*)0x140b5a8a8,*(unsigned long long*)(world.storage.data()+0x58),*(float*)(world.storage.data()+0x3d98),*(float*)(world.storage.data()+0x3d9c),*(float*)(world.storage.data()+0x3da0));{const char* setting=getenv("SINKING_WARM_FRAMES");int frames=setting&&strcmp(setting,"auto")?atoi(setting):(submitted&&(movie||world.effects.events.empty())?1:3);if(frames<1||frames>16)throw std::runtime_error("Warm frames must be between 1 and 16");for(int i=0;i<frames;i++){*(uint64_t*)0x1408852a0=++frameIndex;*(uint64_t*)0x140b6c368=frameIndex;*(uint64_t*)0x140b6c280=frameIndex;*(uint32_t*)0x140b5a0c0=*(uint32_t*)0x140b5a0c4=0;*(uint64_t*)0x140b6c2d0=1;*(uint64_t*)0x140b6cde0=frameIndex;const float dt=movie?effectStep/frames:1.f/30;*(float*)0x140b6c2d8=dt;
   const double clock=world.effects.clock+(movie?i+1-frames:i+1)*dt;
   world.effects.draw(r,jai,world.storage,world.entities,clock+1e-9,dt);
   for(auto p:world.entities){auto e=(uint8_t*)p;if(*(uint64_t*)(e+0x108)!=0x140682ba8ull)continue;
    if(*(uint32_t*)(e+0x100)>=0xffff0000){e[0x4e8]=1;}
   }
   // Reset both CPU and upload cursors for each water pass (0x1402766d1).
   *(uint32_t*)0x140b59e80=*(uint32_t*)0x140b59e84=0;
   *(uint32_t*)0x140b59fa0=*(uint32_t*)0x140b59fa4=0;
   r.image.function<Init>(0x1403765a0)(frame.data(),world.storage.data());
   // Static output: settle the original room spotlight transitions after its
   // first pass has collected the active family's illuminated rooms.
   if(getenv("SINKING_TRACE_PARTICLES")){fprintf(stderr,"Particle list=%llu sourceCount=%llu vertices=%llu dt=%f\n",*(unsigned long long*)0x1410cff30,*(unsigned long long*)0x140b6d310,*(unsigned long long*)0x140b6d318,*(float*)0x140b6c2d8);
    for(auto p:world.entities){auto e=(uint8_t*)p;if(*(uint64_t*)(e+0x108)==0x140682ba8ull)fprintf(stderr,"Particle uid=%u hidden=%u age=%f capacity=%llu updated=%llu rate=%f opacity=%f\n",*(uint32_t*)(e+0x100),e[0x118],*(float*)(e+0x4e0),*(unsigned long long*)(e+0xcc8),*(unsigned long long*)(e+0xd70),*(float*)(e+0x338),*(float*)(e+0x510));}}
   if(getenv("SINKING_DUMP_BUFFERS")){auto shader=**(uint8_t***)0x140b6dac8;auto index=*(uint32_t*)(shader+0x38);auto cb=*(uint8_t**)(frame.data()+0x20+index*40);auto cpu=*(char**)(cb+0x58);auto size=*(uint32_t*)(cb+0x60);std::ofstream f(std::string(getenv("SINKING_DUMP_BUFFERS"))+"/world-"+std::to_string(i)+".bin",std::ios::binary);f.write(cpu,size);}
   auto w=world.storage.data();auto count=*(uint64_t*)(w+0x6d88);auto rooms=*(uint8_t**)(w+0x6d90);fprintf(stderr,"Spotlight rooms=%llu\n",count);for(uint64_t j=0;j<count;j++){memcpy(rooms+j*16,rooms+j*16+4,4);fprintf(stderr,"Room x=%d y=%d visibility=%g\n",*(int*)(rooms+j*16+8),*(int*)(rooms+j*16+12),*(float*)(rooms+j*16));}
}}fprintf(stderr,"Original scene recording completed, reading final target drawCount=%llu matrixCount=%u paramCount=%u\n",*(unsigned long long*)0x140b6c2c4,*(unsigned*)0x140b5a780,*(unsigned*)0x140b5a8a8);
  uint8_t transitioned=0;auto data=*(uint8_t**)(ot+0x60);OriginalGPUBuffers::transition(jai.data(),data,D3D12_RESOURCE_STATE_COPY_SOURCE,~0u,&transitioned);D3D12_TEXTURE_COPY_LOCATION dst={};dst.pResource=readback;dst.Type=D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;dst.PlacedFootprint.Footprint={DXGI_FORMAT_R8G8B8A8_UNORM,width,height,1,pitch};D3D12_TEXTURE_COPY_LOCATION src={};src.pResource=*(ID3D12Resource**)data;src.Type=D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;commands.list->CopyTextureRegion(&dst,0,0,0,&src,nullptr);
  struct Dump{std::string name;ID3D12Resource* buffer;UINT64 size;};std::vector<Dump> dumps;
  if(getenv("SINKING_DUMP_TARGETS"))for(auto name:{"main_color_target","main_color_target_sample","ldr_target","final_target","atmosphere_transmittance"}){
   auto it=debugTargets.find(name);if(it==debugTargets.end())continue;auto td=*(uint8_t**)(it->second+0x60);auto resource=*(ID3D12Resource**)td;D3D12_RESOURCE_DESC desc={};using GetDesc=D3D12_RESOURCE_DESC* (STDMETHODCALLTYPE *)(ID3D12Resource*,D3D12_RESOURCE_DESC*);((GetDesc)(*(void***)resource)[10])(resource,&desc);D3D12_PLACED_SUBRESOURCE_FOOTPRINT footprint;UINT rows;UINT64 rowSize,size;host.d->GetCopyableFootprints(&desc,0,1,0,&footprint,&rows,&rowSize,&size);if(desc.Format==DXGI_FORMAT_R11G11B10_FLOAT){footprint.Footprint.RowPitch=UINT(desc.Width)*16;size=UINT64(footprint.Footprint.RowPitch)*desc.Height;}auto buffer=host.buffer(size,D3D12_HEAP_TYPE_READBACK);uint8_t changed=0;OriginalGPUBuffers::transition(jai.data(),td,D3D12_RESOURCE_STATE_COPY_SOURCE,~0u,&changed);D3D12_TEXTURE_COPY_LOCATION a={},b={};a.pResource=buffer;a.Type=D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;a.PlacedFootprint=footprint;b.pResource=resource;b.Type=D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;commands.list->CopyTextureRegion(&a,0,0,0,&b,nullptr);dumps.push_back({name,buffer,size});fprintf(stderr,"Dump %s format=%u pitch=%u size=%llu\n",name,desc.Format,footprint.Footprint.RowPitch,size);
  }
  if(auto* catalog=OriginalTextureCatalog::active){catalog->upload.flush();if(profile)fprintf(stderr,"Textures still deferred: %zu\n",catalog->pending.size());}if(profile&&DeferredPipelines::enabled)fprintf(stderr,"Graphics pipelines: %zu of %zu compiled\n",DeferredPipelines::registered-DeferredPipelines::pending.size(),DeferredPipelines::registered);if(profile)recorded=Clock::now();commands.wait();if(profile)completed=Clock::now();for(auto&dump:dumps){void*bytes=nullptr;D3D12_RANGE range={0,(SIZE_T)dump.size};check(dump.buffer->Map(0,&range,&bytes),"Map debug target");std::ofstream f(std::string(getenv("SINKING_DUMP_TARGETS"))+"-"+dump.name+".bin",std::ios::binary);f.write((char*)bytes,dump.size);D3D12_RANGE none={0,0};dump.buffer->Unmap(0,&none);}submitted=true;check(host.d->GetDeviceRemovedReason(),"Full scene device status");void*p=nullptr;D3D12_RANGE range={0,size_t(pitch)*height};check(readback->Map(0,&range,&p),"Map final image");slot.write(getenv("SINKING_RAW_OUTPUT")?getenv("SINKING_RAW_OUTPUT"):"original-frame.rgba",p,pitch);if(profile){auto ms=[](auto a,auto b){return std::chrono::duration<double,std::milli>(b-a).count();};fprintf(stderr,"FrameProfile record_ms=%.3f submit_wait_ms=%.3f read_write_ms=%.3f\n",ms(begin,recorded),ms(recorded,completed),ms(completed,Clock::now()));}D3D12_RANGE none={0,0};readback->Unmap(0,&none);fprintf(stderr,"Original scene command submission completed\n");
 }
};
