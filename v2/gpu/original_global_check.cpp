#include <iostream>
#include "original_frame.h"
#include "original_descriptor_heaps.h"
static void __attribute__((ms_abi)) missingMesh(void*,uint64_t*name,void**,uint8_t*){fprintf(stderr,"Unconnected mesh request: %.*s\n",int(name[0]),(char*)name[1]);_Exit(3);}
int main(int argc,char**argv){try{if(argc!=7&&argc!=8)throw std::runtime_error("Usage: original-global-check EXE CATALOG D3DMetal TEXTURES MATERIALS MESHES [WORLD]");Host host(argv[3]);OriginalRuntime runtime(argv[1],host);Bytes ctx;runtime.context(ctx);memcpy((void*)0x140807260ull,ctx.data(),0x280);initializeOriginalHeaps(host,runtime);OriginalTextureCatalog textures(host,argv[4]);if(!textures.pending.empty()&&!getenv("SINKING_RENDER_DEBUG"))DeferredPipelines::install(runtime.image,host);Catalog catalog(argv[2],runtime,host);MaterialCatalog materials(argv[5],runtime,host);MeshCatalog meshes(argv[6],runtime,host);*(uint8_t*)0x140805fd4=1;*(uint8_t*)0x140805fd5=0;*(uint8_t*)0x1410d0242=1;using F=void (__attribute__((ms_abi)) *)(uint8_t);runtime.image.function<F>(0x1401b8a00)(0);{std::string name="editorUI_missing";if(meshes.assets.count(name)){uint64_t str[]={name.size(),(uintptr_t)name.data()};void*mesh=nullptr;uint8_t ok=0;MeshCatalog::lookup(ctx.data(),str,&mesh,&ok);*(void**)0x140b6cdd8=mesh;}}puts("Original global resource initialization completed");if(const char* probe=getenv("SINKING_CHECK_MATERIALS")){
 std::ifstream list(probe);if(!list)throw std::runtime_error("Missing material probe list");
 std::string name;unsigned count=0;while(std::getline(list,name)){if(!materials.assets.count(name)||!materials.load(ctx.data(),name))throw std::runtime_error("Catalog material unavailable: "+name);++count;}
 printf("Verified %u catalog materials with original initializer\n",count);return 0;
}if(argc==8){OriginalWorld world(argv[7],runtime,ctx);std::string effectPath=argv[7];const auto suffix=effectPath.find_last_of('.');if(suffix!=std::string::npos)effectPath.resize(suffix);effectPath+=".effects";world.effects.load(effectPath.c_str(),runtime,ctx);printf("Original world loaded: %zu entities\n",world.entities.size());if(getenv("SINKING_RENDER_FRAME")){if(!getenv("SINKING_SERVE")){std::string snapshot=argv[7];auto dot=snapshot.find_last_of('.');if(dot!=std::string::npos)snapshot.resize(dot);snapshot+=".snapshot";if(std::ifstream(snapshot))world.update(snapshot.c_str());}OriginalFrame frame(host,runtime,ctx,world,!getenv("SINKING_SERVE"));if(getenv("SINKING_SERVE")){std::string line;std::cout<<"READY"<<std::endl;while(std::getline(std::cin,line)){auto tab=line.find('\t');if(tab==std::string::npos)throw std::runtime_error("Expected snapshot and output paths");using Clock=std::chrono::steady_clock;
 const bool profile=getenv("SINKING_FRAME_PROFILE")!=nullptr;
 Clock::time_point start,updated;
 if(profile)start=Clock::now();
 world.update(line.substr(0,tab).c_str());
 if(profile)updated=Clock::now();
 if(getenv("SINKING_MOVIE"))world.moviePose(line.substr(0,tab)+".pose");
 if(profile){using Ms=std::chrono::duration<double,std::milli>;fprintf(stderr,"SceneProfile update_ms=%.3f pose_ms=%.3f\n",Ms(updated-start).count(),Ms(Clock::now()-updated).count());}
 setenv("SINKING_RAW_OUTPUT",line.substr(tab+1).c_str(),1);frame.draw();std::cout<<"DONE"<<std::endl;}}}}}catch(const std::exception&e){fprintf(stderr,"%s\n",e.what());return 1;}}
