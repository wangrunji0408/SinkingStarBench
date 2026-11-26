#pragma once
#include "original_mesh_catalog.h"
#include "original_layers.h"
#include "original_effects.h"
struct OriginalWorld {
 OriginalEffects effects;
 // Topology changes create a new worker. Entity addresses stay valid between draws.
 std::unordered_map<uint32_t,uint8_t*> byId;
 struct SnapshotRow {uint32_t id;float values[18];};
 static_assert(sizeof(SnapshotRow)==76);
 std::vector<SnapshotRow> seedRows;
 std::vector<uint8_t*> seedEntities;
 std::vector<Bytes> beamSegments;std::vector<std::pair<uint8_t*,uint8_t*>> beams;uint32_t activeFamily=0;std::vector<Bytes> mirageBlocks,mirageSegments;std::vector<void*> miragePointers;
 Bytes storage=Bytes(32640),grid=Bytes(128);std::vector<void*> entities;Bytes idlePlayer=Bytes(352),idlePose;std::map<std::string,Bytes> posePlayers,poses;
 struct BardRange{uint32_t emitter,id;float strength;float layers[5][2];};
 static_assert(sizeof(BardRange)==0x34);
 std::vector<BardRange> bardRanges;
 std::vector<uint8_t*> charmedProps;
 // The water pass (0x1402ae890) reads these directly from the world's
 // Temporary_Splash list. The editor marker mesh plays no part in the effect.
 struct Splash {
  uint8_t unused[0xa0];
  float position[3];
  uint8_t padding[0x330-0xac];
  float parameters[4];
  uint64_t randomState[2];
 };
 static_assert(sizeof(Splash)==0x350 && offsetof(Splash,randomState)==0x340);
 std::vector<Splash> splashes;
 std::vector<Splash*> splashPointers;
 void updateCharmDisplay(){
  // Original visual update for Rock and Mirror: fade in, float and wobble.
  // The authored transform is restored from the snapshot before each draw.
  using Visual=void (__attribute__((ms_abi)) *)(void*,float);
  for(auto e:charmedProps)((Visual)0x14030a3e0ull)(e,1.0f);
 }
 // Original visual update (0x140409890, sky branch 0x14041eef0):
 // Sky materials encode physical brightness in output tint.x, not authored tint.
 static void updateSky(uint8_t* e){
  if(*(uint64_t*)(e+0x108)!=0x14069f390ull)return;
  fprintf(stderr,"Sky uid=%u hidden=%u visible=%u brightness=%g\n",*(unsigned*)(e+0x100),e[0x118],e[0x330],*(float*)(e+0x334));float tint[]={*(float*)(e+0x334),1.0f,1.0f,1.0f};memcpy(e+0x1b0,tint,sizeof(tint));
 }
 // Seed the settled room mask before the first render pass (3ac170 consumes
 // it before 376b50 updates transitions). Resolve room IDs with the EXE helper.
 void selectRooms(OriginalRuntime&r,Bytes&ctx){
  // Level-load initialization at 0x140356210: grid_overlay divides by these
  // original room dimensions even when no powered rooms are present.
  using RoomDimensions=void (__attribute__((ms_abi)) *)(void*,void*,void*);
  // The EXE returns dimensions, origin and room ID: 24 bytes, not just a Vec2.
  uint8_t dimensions[24]={};r.image.function<RoomDimensions>(0x1402df740)(ctx.data(),storage.data(),dimensions);memcpy(storage.data()+0x7de4,dimensions,8);

  auto w=storage.data();uint64_t selected[2]={};using Select=void (__attribute__((ms_abi)) *)(void*,void*,uint8_t,void*);r.image.function<Select>(0x14031d8c0)(ctx.data(),w,1,selected);
  *(uint64_t*)(w+0x6d88)=0;auto capacity=*(uint64_t*)(w+0x6d98);if(capacity<selected[0]){void*roomData=nullptr;OriginalRuntime::allocate(ctx.data(),0,std::max<uint64_t>(16,selected[0])*16,0,nullptr,0,&roomData);*(void**)(w+0x6d90)=roomData;*(uint64_t*)(w+0x6d98)=std::max<uint64_t>(16,selected[0]);}
  for(uint64_t i=0;i<selected[0];i++){int index=-1,x=0,y=0;int layer=0;using Room=void (__attribute__((ms_abi)) *)(void*,void*,void*,void*,void*,void*,void*);auto entity=((uint8_t**)selected[1])[i];r.image.function<Room>(0x140294910)(ctx.data(),w,entity+0xa0,&index,&x,&y,&layer);if(index<0)continue;auto count=*(uint64_t*)(w+0x6d88);auto data=*(uint8_t**)(w+0x6d90);bool found=false;for(uint64_t j=0;j<count;j++)if(*(int*)(data+j*16+8)==x&&*(int*)(data+j*16+12)==y)found=true;if(found)continue;float visible=1;memcpy(data+count*16,&visible,4);memcpy(data+count*16+4,&visible,4);memcpy(data+count*16+8,&x,4);memcpy(data+count*16+12,&y,4);*(uint64_t*)(w+0x6d88)=count+1;}
 }
 // Match sky selection in 0x14025a9a0 without enabling editor rendering.
 void selectSkies(OriginalRuntime&r,Bytes&ctx){
  auto w=storage.data();for(int i=0;i<6;i++)*(void**)(w+0xd0+i*0x80)=nullptr;
  using Layer=void (__attribute__((ms_abi)) *)(void*,void*,uint8_t*);
  for(auto entity:entities){auto e=(uint8_t*)entity;if(*(uint64_t*)(e+0x108)!=0x14069f390ull||e[0x118])continue;
   uint8_t layer=0;r.image.function<Layer>(0x140251400)(ctx.data(),e,&layer);
   if(w[0x63a0] && !((w[0x63c0]>2)!=(layer<3) && layer==w[0x63c0]))continue;
   if(*(int*)(w+0x64a8)<2)layer=0;
   if(layer<6)*(void**)(w+0xd0+layer*0x80)=e;fprintf(stderr,"Selected sky uid=%u layer=%u active=%u multi=%u count=%d\n",*(unsigned*)(e+0x100),layer,w[0x63c0],w[0x63a0],*(int*)(w+0x64a8));
  }
 }
 // 0x1402d02ab: each secondary fog volume with create_overhang owns a
 // render bucket. Register the volume before assigning meshes to buckets.
 static uint8_t* appendRecord(Bytes&ctx,uint8_t*array,size_t stride){
  auto&count=*(uint64_t*)array;auto&data=*(void**)(array+8);auto&cap=*(uint64_t*)(array+16);
  if(count==cap){auto next=std::max<uint64_t>(8,cap*2);
   OriginalRuntime::allocate(ctx.data(),data?1:0,next*stride,cap*stride,data,0,&data);cap=next;}
  auto record=(uint8_t*)data+count++*stride;memset(record,0,stride);return record;
 }
 void registerOverhangs(OriginalRuntime&r,Bytes&ctx){
  using Query=void (__attribute__((ms_abi)) *)(void*,void*,void*);
  for(auto p:entities){auto e=(uint8_t*)p;
   if(*(uint64_t*)(e+0x108)!=0x14068be10ull||e[0x378]||!e[0x374])continue;
   uint8_t layer=0;r.image.function<Query>(0x140251400)(ctx.data(),e,&layer);
   auto buckets=storage.data()+0x58+layer*0x80;
   auto index=*(uint64_t*)buckets;auto bucket=appendRecord(ctx,buckets,0x1c8);
   bucket[0]=layer;*(int32_t*)(bucket+4)=-1;bucket[0xd]=1;
   auto volume=appendRecord(ctx,buckets+0x28,0x24);
   r.image.function<Query>(0x1400d1e40)(ctx.data(),e,volume);
   *(float*)(volume+0x18)=1;*(int32_t*)(volume+0x1c)=int32_t(index);
   volume[0x20]=e[0x377];volume[0x21]=e[0x376];
  }
 }
 double fogClock=-1;
 // Resolve the active floor before camera, sky, fog and beam preparation.
 void selectCamera(OriginalRuntime&r,Bytes&ctx){
  auto w=storage.data();using Layer=void (__attribute__((ms_abi)) *)(void*,void*,uint8_t*);
  uint8_t*camera=nullptr;uint8_t*hero=nullptr;for(auto p:entities){auto e=(uint8_t*)p;if(*(uint64_t*)(e+0x108)==0x1406a8bf0ull && *(uint32_t*)(e+0x360)==activeFamily){hero=e;r.image.function<Layer>(0x140251400)(ctx.data(),e,w+0x63c0);break;}}
  for(auto p:entities){auto e=(uint8_t*)p;if(*(uint64_t*)(e+0x108)!=0x1406885b8ull||e[0x358]||*(int64_t*)(e+0x370)!=0)continue;uint8_t layer=0;r.image.function<Layer>(0x140251400)(ctx.data(),e,&layer);if(w[0x63a0]&&layer!=w[0x63c0])continue;
   camera=e;memcpy(w+0x3d98,e,12);memcpy(w+0x3db0,e+0x18,16);memcpy(w+0x3dc0,e+0x340,12);*(float*)(w+0x3dc0)*=0.017453292519943295f;memcpy(w+0x3dd8,e+0x378,8);auto settings=**(uint8_t***)(w+0xdd8);
   if(settings[0x3a2]&&hero){
    float center[3]={},minimum[2]={},maximum[2]={};int roomX=0,roomY=0;
    // 2654e0 uses the authored per-screen origin (unlike the fog grid helper).
    using RoomCamera=void (__attribute__((ms_abi)) *)(void*,float,float,void*,void*,void*,int*,int*);
    r.image.function<RoomCamera>(0x140260630)(w,*(float*)(hero+0xa0),*(float*)(hero+0xa4),center,minimum,maximum,&roomX,&roomY);
    center[2]=*(float*)(w+0x64c8+w[0x63c0]*12);memcpy(w+0x3d90,center,8);memcpy(w+0x3de0,center,8);memcpy(w+0x3dfc,center,8);memcpy(w+0x3e04,minimum,8);memcpy(w+0x3e0c,maximum,8);w[0x3df8]=1;
    float pos[]={center[0],center[1]-*(float*)(e+0x364),center[2]+*(float*)(e+0x360)};
    float target[]={center[0],center[1]-*(float*)(e+0x368),center[2]},up[]={0,0,1};float matrix[16]={},q[4]={};
    using Look=void (__attribute__((ms_abi)) *)(void*,void*,void*,void*);r.image.function<Look>(0x1400f86b0)(pos,target,up,matrix);using Quat=void (__attribute__((ms_abi)) *)(void*,void*);r.image.function<Quat>(0x1400f4a10)(matrix,q);for(int i=0;i<3;i++)q[i]=-q[i];memcpy(w+0x3d98,pos,12);memcpy(w+0x3db0,q,16);
   }else{using Camera=void (__attribute__((ms_abi)) *)(void*,void*,void*);r.image.function<Camera>(0x140303dd0)(e,w+0x3d98,w+0x3db0);}
   fprintf(stderr,"Camera selected uid=%u layer=%u\n",*(uint32_t*)(e+0x100),layer);break;
  }
  {int total=0,nx=0,ny=0,layers=0;using Counts=void (__attribute__((ms_abi)) *)(void*,void*,int*,int*,int*,int*);r.image.function<Counts>(0x1402601c0)(ctx.data(),w,&total,&nx,&ny,&layers);*(int64_t*)(w+0x7df0)=nx*ny;*(int64_t*)(w+0x7df8)=total;*(int64_t*)(w+0x7e00)=layers;*(int64_t*)(w+0x7e08)=nx;*(int64_t*)(w+0x7e10)=ny;}
  // 0x14041b850: shaders read the current and previous reveal alpha,
  // independently of the explored-state byte. Advance once per game time delta.
  const bool settle=fogClock<0||!getenv("SINKING_MOVIE");
  float increment=float(std::max(0.,effects.clock-fogClock))*(*(float*)0x14080541c);
  fogClock=effects.clock;
  if(hero)for(auto p:entities){
   auto e=(uint8_t*)p;if(*(uint64_t*)(e+0x108)!=0x14068be10ull)continue;
   uint8_t layer=0;r.image.function<Layer>(0x140251400)(ctx.data(),e,&layer);if(layer!=w[0x63c0])continue;
   auto states=*(uint8_t**)(e+0x3b8);
   if(e[0x378]){
    uint8_t changed=0;using Clear=void (__attribute__((ms_abi)) *)(void*,void*,void*,void*,void*);
    auto clear=r.image.function<Clear>(0x14028a370);clear(ctx.data(),w,e,hero,&changed);
    auto clears=*(uint64_t*)(w+0x768);auto markers=*(uint8_t***)(w+0x770);
    for(uint64_t i=0;i<clears;i++)if(*(float*)(markers[i]+0x330)==0&&!markers[i][0x334])clear(ctx.data(),w,e,markers[i],&changed);
    auto regions=*(uint8_t**)(e+0x410);
    for(uint64_t i=0;i<*(uint64_t*)(e+0x3b0);i++)if(states[i]==1){
     auto alpha=(float*)(regions+i*0x20+0x14);alpha[1]=settle?1:alpha[0];
     alpha[0]=settle?1:std::min(1.f,alpha[0]+increment);if(alpha[0]>=1)states[i]=2;
    }
   }else{
    auto pos=(float*)e,player=(float*)hero,extent=(float*)(e+0x37c);
    if(!states[0]&&player[0]>=pos[0]-extent[0]&&player[0]<pos[0]+extent[0]&&
       player[1]>=pos[1]-extent[1]&&player[1]<pos[1]+extent[1]&&
       player[2]>=pos[2]-extent[2]&&player[2]<pos[2]+extent[2]){
     states[0]=1;memcpy(e+0x430,player,8);
    }
    if(states[0]==1){auto alpha=(float*)(e+0x438);alpha[1]=settle?1:alpha[0];
     alpha[0]=settle?1:std::min(1.f,alpha[0]+increment);if(alpha[0]>=1)states[0]=2;}
   }
  }
  if(camera&&(**(uint8_t***)(w+0xdd8))[0x3a2]){using Follow=void (__attribute__((ms_abi)) *)(void*,void*,void*,void*,float,uint8_t);r.image.function<Follow>(0x14043e500)(w,w+0x3d98,w+0x656c,camera,1,0);*(float*)(w+0x3da0)+=*(float*)(w+0x64c8+w[0x63c0]*12);}
  for(auto pair:beams){using Visual=void (__attribute__((ms_abi)) *)(void*,void*,void*);r.image.function<Visual>(0x1403a8150)(pair.first,pair.second,nullptr);}
  auto old=w[0x3ee8];w[0x3ee8]=1;using Exposure=void (__attribute__((ms_abi)) *)(void*,float,uint8_t);r.image.function<Exposure>(0x14042a9e0)(w,0,1);w[0x3ee8]=old;
 }
 static uint64_t read64(Reader&r){uint64_t n;r.f.read((char*)&n,8);if(!r.f)throw std::runtime_error("Truncated world");return n;}
 OriginalWorld(const char*path,OriginalRuntime&r,Bytes&ctx){
  using Ctor=void (__attribute__((ms_abi)) *)(void*);r.image.function<Ctor>(0x14007cff0)(storage.data());const char*level=getenv("SINKING_LEVEL");if(!level)level="heroes1_2_v2";*(const char**)(storage.data()+8)=level;*(uint64_t*)storage.data()=strlen(level);*(void**)(ctx.data()+0x90)=storage.data();*(void**)0x1408072f0=storage.data();*(void**)0x140b6cd58=storage.data();*(void**)(storage.data()+0x50)=grid.data();*(void**)(storage.data()+0x3ef0)=storage.data();
  using Meta=void (__attribute__((ms_abi)) *)(void*,uint8_t);for(unsigned i=0;i<88;i++){auto p=(uint64_t*)(0x140b5dfa8ull+i*240);p[0]=p[1]=((uint64_t*)0x140732dc0ull)[i];r.image.function<Meta>(0x14019deb0)(p,0);}auto p=(uint64_t*)0x140884e60ull;p[0]=p[1]=0x1406acc30ull;r.image.function<Meta>(0x14019deb0)(p,1);
  for(auto base:{0x140884e70ull,0x140884ee8ull}){auto count=*(uint64_t*)base;auto entries=*(uint8_t***)(base+8);for(uint64_t i=0;i<count;i++)entries[i][0x88]=1;}
  using Init=void (__attribute__((ms_abi)) *)();r.image.function<Init>(0x1403e5b90)();
  Reader in(path);auto baseVersion=read64(in);auto count=in.word();byId.reserve(count);for(unsigned i=0;i<count;i++){auto uid=in.word();auto type=read64(in),version=read64(in);auto body=in.blob();void*entity=nullptr;using Create=void (__attribute__((ms_abi)) *)(void*,void*,uint64_t,uint32_t,void**);r.image.function<Create>(0x140409400)(ctx.data(),storage.data(),type,uid,&entity);auto e=(uint8_t*)entity;auto handle=*(uint64_t*)(e+0xf8);auto ctor=*(uintptr_t*)(type+0x78);if(ctor)r.image.function<Ctor>(ctor)(entity);*(uint64_t*)(e+0xf8)=handle;*(void**)(e+0xf0)=storage.data();*(uint64_t*)(e+0x108)=type;*(uint32_t*)(e+0x100)=uid;
   uint64_t data[]={body.size(),(uintptr_t)body.data()};uint8_t ok=0;using Apply=void (__attribute__((ms_abi)) *)(void*,uint64_t,void*,uint64_t,uint64_t*,uint64_t,uint64_t,uint64_t,uint8_t,uint8_t*);r.image.function<Apply>(0x1402386b0)(ctx.data(),type,entity,0,data,baseVersion,version,0,1,&ok);if(!ok)throw std::runtime_error("Original entity diff failed for uid="+std::to_string(uid));if(auto mesh=*(uint8_t**)(e+0x88)){auto md=*(uint8_t**)(mesh+0x38);auto count=*(uint64_t*)(md+0x38);auto oldCount=*(uint64_t*)(e+0x90);auto old=*(void**)(e+0x98);if(old&&count!=oldCount){void*slots=nullptr;OriginalRuntime::allocate(ctx.data(),0,count*8,0,nullptr,0,&slots);memcpy(slots,old,std::min(count,oldCount)*8);*(uint64_t*)(e+0x90)=count;*(void**)(e+0x98)=slots;fprintf(stderr,"Normalized material slots for uid=%u: %llu -> %llu\n",uid,oldCount,count);}}memcpy(e+0x1b0,e+0x64,16);memcpy(e+0x1c0,e+0x74,16);memcpy(e+0xa0,e,12);memcpy(e+0x1d0,e+0xa0,12);memcpy(e+0x1dc,e+0x18,16);memcpy(e+0x1ec,e+0xc,12);updateSky(e);entities.push_back(entity);byId[*(uint32_t*)(e+0x100)]=e;
  }
  for(auto entity:entities){auto e=(uint8_t*)entity;if(*(uint64_t*)(e+0x108)==0x1406885b8ull && *(int64_t*)(e+0x370)!=-1 && !e[0x358]){memcpy(storage.data()+0x3d98,e,12);memcpy(storage.data()+0x3db0,e+0x18,16);memcpy(storage.data()+0x3dc0,e+0x340,12);*(float*)(storage.data()+0x3dc0)*=0.017453292519943295f;memcpy(storage.data()+0x3dd8,e+0x378,8);fprintf(stderr,"Authored camera fov=%g near=%g far=%g\n",*(float*)(e+0x340),*(float*)(e+0x344),*(float*)(e+0x348));break;}}

  std::unordered_set<std::string> missingIdle;
  for(auto entity:entities){auto e=(uint8_t*)entity;auto mesh=*(uint8_t**)(e+0x88);if(!mesh)continue;std::string name(*(char**)(mesh+8),*(uint64_t*)mesh);if(!poses.count(name)){if(*(uint64_t*)(e+0x108)!=0x1406a8bf0ull && (missingIdle.count(name)||!std::ifstream("poses/"+name+".bin"))){missingIdle.insert(name);continue;}std::string path="poses/"+name+".bin";Reader pose(path.c_str());auto bones=pose.word();auto&data=poses[name];data.resize(bones*48);pose.f.read((char*)data.data(),data.size());if(!pose.f)throw std::runtime_error("Truncated idle pose");auto&player=posePlayers[name];player.resize(352);*(uint64_t*)(player.data()+0xf0)=bones;*(void**)(player.data()+0xf8)=data.data();}*(void**)(e+0x120)=posePlayers[name].data();*(uint32_t*)(e+0xd8)|=0x20;}

  using Register=void (__attribute__((ms_abi)) *)(void*,void*,void*);for(auto entity:entities){auto e=(uint8_t*)entity;*(int32_t*)(e+0x110)=-1;*(int32_t*)(e+0x114)=-1;r.image.function<Register>(0x140326f40)(ctx.data(),storage.data(),entity);}
  auto settings=*(uint8_t***)(storage.data()+0xdd8);if(!settings||!*(uint64_t*)(storage.data()+0xdd0))throw std::runtime_error("Missing Settings");Bytes scratch(4096);using Bounds=void (__attribute__((ms_abi)) *)(void*,void*,void*);r.image.function<Bounds>(0x1402b3b30)(ctx.data(),*settings,scratch.data());using Setup=void (__attribute__((ms_abi)) *)(void*,void*,uint8_t);r.image.function<Setup>(0x140404b50)(ctx.data(),storage.data(),1);
  if(getenv("SINKING_DUMP_WALLS"))for(auto off:{0x3e98,0x3ec0}){auto n=*(uint64_t*)(storage.data()+off);auto data=*(char**)(storage.data()+off+8);std::ofstream f(std::string(getenv("SINKING_DUMP_WALLS"))+"-"+std::to_string(off)+".bin",std::ios::binary);f.write(data,n*8);}
  uint8_t continuation[14];memcpy(continuation,(void*)0x140438d64,14);r.image.redirect(0x140438d64,(void*)originalLayerEnd);originalSetupSection(ctx.data(),storage.data(),0x1404380cf);r.image.patch(0x140438d64,continuation,14);storage[0x63a0]=*(int*)(storage.data()+0x64a8)>1;fprintf(stderr,"LAYERS count=%d\n",*(int*)(storage.data()+0x64a8));
  auto nx=*(uint64_t*)(*settings+0x330),ny=*(uint64_t*)(*settings+0x338);if(nx*ny>1000000)throw std::runtime_error("Invalid grid");auto g=(uint64_t*)grid.data();g[0]=nx;g[1]=ny;g[2]=nx*ny;memcpy(grid.data()+0x1c,*settings+0x34c,12);void*cells=nullptr;OriginalRuntime::allocate(ctx.data(),0,nx*ny*40,0,nullptr,0,&cells);*(void**)(grid.data()+0x30)=cells;
  for(auto e:entities){*(uint32_t*)((uint8_t*)e+0xd8)|=2;r.image.function<Bounds>(0x140324160)(ctx.data(),grid.data(),e);}*(void**)0x1410d7000=*(void**)(ctx.data()+0x10);*(uint64_t*)0x1410d7008=*(uint64_t*)(ctx.data()+0x18);using OceanMesh=void (__attribute__((ms_abi)) *)(void*,void*);for(auto p:entities){auto e=(uint8_t*)p;if(*(uint64_t*)(e+0x108)==0x14068d378ull&&!*(void**)(e+0x88)){fprintf(stderr,"Building original ocean mesh uid=%u\n",*(unsigned*)(e+0x100));r.image.function<OceanMesh>(0x14016a6a0)(ctx.data(),e);}}for(auto p:entities){auto e=(uint8_t*)p;if(*(uint64_t*)(e+0x108)==0x140693b78ull&&!*(void**)(e+0x88))r.image.function<OceanMesh>(0x140218340)(ctx.data(),e);}using FogInit=void (__attribute__((ms_abi)) *)(void*,void*,uint8_t);for(auto p:entities){auto e=(uint8_t*)p;if(*(uint64_t*)(e+0x108)==0x14068be10ull)r.image.function<FogInit>(0x140171840)(ctx.data(),e,0);}using Buckets=void (__attribute__((ms_abi)) *)(void*,void*);r.image.function<Buckets>(0x14024cc90)(ctx.data(),storage.data());registerOverhangs(r,ctx);using RenderRegister=void (__attribute__((ms_abi)) *)(void*,void*,void*,uint8_t);for(auto entity:entities)r.image.function<RenderRegister>(0x1403ad390)(ctx.data(),storage.data(),entity,1);storage[0x368]=1;size_t meshCount=0;for(auto entity:entities)if(auto m=*(uint8_t**)((uint8_t*)entity+0x88)){meshCount++;if(meshCount<4)fprintf(stderr,"Entity mesh: %.*s loaded=%u data=%p\n",int(*(uint64_t*)m),*(char**)(m+8),m[0x20],*(void**)(m+0x38));}fprintf(stderr,"Original entity mesh references: %zu\n",meshCount);fprintf(stderr,"Original world spatial setup: %llu x %llu\n",(unsigned long long)nx,(unsigned long long)ny);
  // Level loading normally builds the filtered ocean list used to place
  // temporary splashes on the visible water layer.
  r.image.function<Buckets>(0x14039ce10)(ctx.data(),storage.data());
  for(auto p:entities){auto e=(uint8_t*)p;if(*(uint64_t*)(e+0x108)==0x1406885b8ull)fprintf(stderr,"Camera uid=%u index=%lld overview=%u exposure=%g manual=%u\n",*(unsigned*)(e+0x100),*(long long*)(e+0x370),e[0x358],*(float*)(e+0x388),e[0x385]);}auto initializing=storage[0x3ee8];storage[0x3ee8]=1;using Exposure=void (__attribute__((ms_abi)) *)(void*,float,uint8_t);r.image.function<Exposure>(0x14042a9e0)(storage.data(),0.0f,1);storage[0x3ee8]=initializing;fprintf(stderr,"Original exposure EV=%g manual=%g\n",*(float*)(storage.data()+0x6cd0),*(float*)(storage.data()+0x6d78));
 }
 std::map<unsigned,Bytes> moviePlayers,movieBones;
 void moviePose(const std::string& path){
  Reader in(path.c_str());auto count=in.word();
  for(unsigned i=0;i<count;i++){auto id=in.word(),n=in.word();if(!byId.count(id)||!n||n>4096)throw std::runtime_error("Invalid movie pose");
   auto e=byId.at(id);auto &bones=movieBones[id],&player=moviePlayers[id];
   bones.resize(n*48);in.f.read((char*)bones.data(),bones.size());if(!in.f)throw std::runtime_error("Truncated movie pose");
   player.resize(352);*(uint64_t*)(player.data()+0xf0)=n;*(void**)(player.data()+0xf8)=bones.data();*(void**)(e+0x120)=player.data();*(uint32_t*)(e+0xd8)|=0x20;
  }
  if(in.f.peek()!=EOF){
   auto tick=in.word(),count=in.word();
   if(count>100000)throw std::runtime_error("Invalid splash count");
   splashes.resize(count);
   splashPointers.resize(count);
   for(unsigned i=0;i<count;i++){
    auto& s=splashes[i];
    in.f.read((char*)s.position,sizeof s.position);
    in.f.read((char*)s.parameters,sizeof s.parameters);
    in.f.read((char*)s.randomState,sizeof s.randomState);
    splashPointers[i]=&s;
   }
   if(!in.f)throw std::runtime_error("Truncated movie effects");
   *(uint64_t*)(storage.data()+0xf60)=count;
   *(void**)(storage.data()+0xf68)=splashPointers.data();
   *(uint64_t*)(storage.data()+0xf70)=count;
   double seconds=tick/60.0;
   if(in.f.peek()!=EOF){in.f.read((char*)&seconds,sizeof seconds);
    if(!in.f||!std::isfinite(seconds)||seconds<0)throw std::runtime_error("Invalid movie clock");}
   *(double*)(storage.data()+0x67d8)=seconds;
   // Original visual update 0x140409890 wraps this separate clock at 1000 s.
   // Level_Params.shader_time reads it at 0x140360990, including for ocean flow.
   *(double*)(storage.data()+0x67e0)=seconds>1000?seconds-1000*(std::ceil(seconds/1000)-1):seconds;
  }
 }
 void update(const char*path){
  Reader in(path);auto n=in.word();
  const bool patch=n==0x3150574f; // OWP1: row-indexed overrides of the topology seed.
  if(patch){
   if(seedRows.empty()){
    Reader seed((std::string(path)+".seed").c_str());auto count=seed.word();
    if(count!=entities.size())throw std::runtime_error("Invalid overworld snapshot seed");
    seedRows.resize(count);seed.f.read((char*)seedRows.data(),count*sizeof(SnapshotRow));
    if(!seed.f)throw std::runtime_error("Truncated overworld snapshot seed");
    seedEntities.reserve(count);
    for(const auto& row:seedRows)seedEntities.push_back(byId.at(row.id));
   }
   auto count=in.word();if(count>seedRows.size())throw std::runtime_error("Invalid overworld snapshot patch");
   for(unsigned i=0;i<count;i++){
    auto index=in.word();auto& row=seedRows.at(index);
    in.f.read((char*)row.values,sizeof row.values);
    if(!in.f)throw std::runtime_error("Truncated overworld snapshot patch");
   }
   n=seedRows.size();
  }else if(n!=entities.size())throw std::runtime_error("World topology changed");
  for(unsigned i=0;i<n;i++){
   SnapshotRow row;auto e=(uint8_t*)nullptr;const float* v=nullptr;
   if(patch){e=seedEntities[i];v=seedRows[i].values;}
   else{in.f.read((char*)&row,sizeof row);if(!in.f)throw std::runtime_error("Invalid state snapshot");e=byId.at(row.id);v=row.values;}
   memcpy(e,v,12);memcpy(e+0xc,v+3,12);memcpy(e+0x18,v+6,16);memcpy(e+0x64,v+10,16);memcpy(e+0x74,v+14,16);
   memcpy(e+0xa0,e,12);memcpy(e+0x1d0,e,12);memcpy(e+0x1dc,e+0x18,16);memcpy(e+0x1ec,e+0xc,12);memcpy(e+0x1b0,e+0x64,16);memcpy(e+0x1c0,e+0x74,16);updateSky(e);*(uint64_t*)(e+0x200)=~0ull;
  }
  if(in.f.peek()!=EOF){
   if(in.word()!=0x53544154)throw std::runtime_error("Invalid snapshot render state");
   activeFamily=in.word();auto players=in.word();
   for(unsigned i=0;i<players;i++){auto id=in.word(),family=in.word();if(!byId.count(id))throw std::runtime_error("Unknown player in render state");*(uint32_t*)(byId.at(id)+0x360)=family;}
   if(in.f.peek()!=EOF){
    if(in.word()!=0x4d495252)throw std::runtime_error("Invalid mirror snapshot");
    auto count=in.word();for(auto ptr:entities){auto e=(uint8_t*)ptr;if(*(uint64_t*)(e+0x108)==0x1406a7878ull){*(void**)(e+0x338)=nullptr;*(void**)(e+0x340)=nullptr;*(float*)(e+0x578)=0;}}mirageBlocks.clear();mirageSegments.clear();miragePointers.clear();
    mirageBlocks.reserve(count);mirageSegments.reserve(count);miragePointers.reserve(count);
    for(unsigned j=0;j<count;j++){
     mirageBlocks.emplace_back(24+20*496);auto block=mirageBlocks.back().data();block[0]=1;auto m=block+24;
     in.f.read((char*)m,496);auto origin=in.word(),segments=in.word();if(segments>10000)throw std::runtime_error("Invalid mirror segments");
     mirageSegments.emplace_back(segments*36);auto &sample=mirageSegments.back();in.f.read((char*)sample.data(),sample.size());
     for(auto off:{0xe0,0x108,0x150})memset(m+off,0,40);
     *(uint64_t*)(m+0x108)=segments;*(void**)(m+0x110)=sample.data();*(uint64_t*)(m+0x118)=segments;
     *(void**)(m+0xc8)=byId.count(origin)?byId.at(origin):nullptr;
     auto mirror=byId.find(*(uint32_t*)m);
     if(mirror!=byId.end()){
      *(void**)(mirror->second+0x340)=m;*(float*)(mirror->second+0x578)=1;
      using Dimensions=void (__attribute__((ms_abi)) *)(void*,void*,void*);
      ((Dimensions)0x1402fb240ull)(storage.data(),m,m+0xa0);
      *(float*)(m+0x18)=m[0x1c]?1.0f:0.0f;
      *(float*)(m+0xc0)=*(float*)(m+0xc4)=*(float*)(m+0x18);
      using Visual=void (__attribute__((ms_abi)) *)(void*,void*,void*);
      ((Visual)0x1403a8150ull)(mirror->second,m+0xa8,m);
      if(getenv("SINKING_TRACE_BEAM")){auto seg=*(uint8_t**)(m+0x158);for(unsigned k=0;k<*(uint64_t*)(m+0x150);k++)fprintf(stderr,"Mirror segment %u visible=%u length=%g dims=%g,%g on=%g\n",k,seg[k*120+32],*(float*)(seg+k*120+88),*(float*)(m+160),*(float*)(m+164),*(float*)(m+24));}
      fprintf(stderr,"Mirror visual reflector=%u segments=%llu\n",*(uint32_t*)m,*(unsigned long long*)(m+0x150));
     }
     miragePointers.push_back(block);
    }
    *(uint64_t*)(storage.data()+0x4290)=count;*(void**)(storage.data()+0x4298)=miragePointers.data();*(uint64_t*)(storage.data()+0x42a0)=count;
   }
   if(in.f.peek()!=EOF){
    if(in.word()!=0x4245414d)throw std::runtime_error("Invalid beam snapshot");auto count=in.word();beams.clear();beamSegments.clear();beamSegments.reserve(count);
    for(unsigned j=0;j<count;j++){auto id=in.word(),off=in.word(),origin=in.word(),n=in.word();if(n>10000||!byId.count(id)||!(off==0x360||off==0x398||off==0x4d8||off==0x618||off==0x758))throw std::runtime_error("Invalid beam record");auto e=byId.at(id),b=e+off;in.f.read((char*)b,320);beamSegments.emplace_back(n*36);auto &segments=beamSegments.back();in.f.read((char*)segments.data(),segments.size());for(auto offset:{0x38,0x60,0xa8})memset(b+offset,0,40);memset(b+0xe0,0,96);*(uint64_t*)(b+0x60)=n;*(void**)(b+0x68)=segments.data();*(uint64_t*)(b+0x70)=n;*(void**)(b+0x20)=byId.count(origin)?byId.at(origin):nullptr;*(float*)(b+0x18)=*(float*)(b+0x1c)=b[0x14]?1:0;beams.emplace_back(e,b);}
   }
   if(in.f.peek()!=EOF){
    if(in.word()!=0x47415445)throw std::runtime_error("Invalid gate snapshot");
    auto count=in.word();for(unsigned i=0;i<count;i++){auto id=in.word(),open=in.word();float t;in.f.read((char*)&t,sizeof(t));
     if(!in.f||!byId.count(id)||*(uint64_t*)(byId.at(id)+0x108)!=0x1406920a8ull)throw std::runtime_error("Invalid gate record");
     auto e=byId.at(id);e[0x338]=open;memcpy(e+0x36c,&t,4);
     // The original visual update copies the authored color for the force-field pass.
     memcpy(e+0x3d8,e+0x350,16);
    }
   }
   if(in.f.peek()!=EOF){
    if(in.word()!=0x41555241)throw std::runtime_error("Invalid protection snapshot");
    auto count=in.word();for(unsigned i=0;i<count;i++){auto id=in.word();float fade;in.f.read((char*)&fade,4);
     if(!in.f||!byId.count(id)||*(uint64_t*)(byId.at(id)+0x108)!=0x1406a8bf0ull)throw std::runtime_error("Invalid protected player");
     auto e=byId.at(id);memcpy(e+0x618,&fade,4);
     float fx[]={float(*(uint32_t*)(e+0x360)!=activeFamily && !(*(uint16_t*)(e+0x5c)&4)),float(*(uint16_t*)(e+0x5c)&1),0,0};
     memcpy(e+0x1c0,fx,16);
    }
   }
   if(in.f.peek()!=EOF){
    if(in.word()!=0x47454d53)throw std::runtime_error("Invalid gem snapshot");
    auto count=in.word();for(unsigned i=0;i<count;i++){auto id=in.word();float powered;in.f.read((char*)&powered,4);
     if(!in.f||!byId.count(id)||*(uint64_t*)(byId.at(id)+0x108)!=0x1406837f8ull)throw std::runtime_error("Invalid gem");
     // Gem shader effect parameters from 0x140417940; white here selects the powered material.
     float effect[]={powered,0,0,0};memcpy(byId.at(id)+0x1c0,effect,16);
    }
   }
   bardRanges.clear();charmedProps.clear();
   for(auto ptr:entities){auto e=(uint8_t*)ptr;
    *(uint32_t*)(e+0x11c)=0;
    const auto type=*(uint64_t*)(e+0x108);
    if(type==0x1406a6d68ull)*(float*)(e+0x3bc)=0;
    if(type==0x1406a7878ull)*(float*)(e+0x588)=0;
   }
   if(in.f.peek()!=EOF){
    if(in.word()!=0x42415244)throw std::runtime_error("Invalid Bard snapshot");
    const auto count=in.word(),targets=in.word();
    if(count>byId.size()||targets>byId.size())throw std::runtime_error("Invalid Bard snapshot count");
    for(unsigned i=0;i<count;i++){
     const auto id=in.word();
     if(!byId.count(id)||*(uint64_t*)(byId.at(id)+0x108)!=0x1406a8bf0ull)throw std::runtime_error("Invalid Bard");
     BardRange state{};state.id=id;state.strength=1;state.layers[2][0]=state.layers[2][1]=1;
     bardRanges.push_back(state);
    }
    for(unsigned i=0;i<targets;i++){
     const auto id=in.word(),bard=in.word();
     if(!byId.count(id)||!byId.count(bard))throw std::runtime_error("Invalid charm target");
     auto e=byId.at(id);*(uint32_t*)(e+0x11c)=bard;
     const auto type=*(uint64_t*)(e+0x108);
     if(type==0x1406a6d68ull||type==0x1406a7878ull)charmedProps.push_back(e);
    }
   }
   if(in.f.peek()!=EOF){
    if(in.word()!=0x4d454348)throw std::runtime_error("Invalid mechanism snapshot");
    auto readFloat=[&](){float x;in.f.read((char*)&x,4);if(!in.f||!std::isfinite(x))throw std::runtime_error("Invalid mechanism value");return x;};
    const auto count=in.word();if(count>byId.size())throw std::runtime_error("Invalid mechanism count");
    for(unsigned i=0;i<count;i++){
     const auto id=in.word(),a=in.word(),b=in.word();const auto t=readFloat(),broken=readFloat();auto e=byId.at(id);
     switch(*(uint64_t*)(e+0x108)){
      case 0x14069e0c0ull: { // Switch palette and per-mesh parameters from 0x1402e9fa0.
       e[0x33c]=a;const auto lit=(*(uint32_t*)(e+0x330)&3)!=0;
       auto paint=[&](uintptr_t first,uintptr_t last,float weight,int offset){
        for(int j=0;j<4;j++)((float*)(e+offset))[j]=((float*)last)[j]+weight*(((float*)first)[j]-((float*)last)[j]);
       };
       paint(0x1408053d4,0x1408053e4,lit?1:0,0x1b0);paint(0x1408053f4,0x140805404,a?1:0,0x1c0);
       break;
      }
      case 0x1406a9158ull: { // Door glow in the original visual update (0x140412f13).
       e[0x330]=a;e[0x331]=b;auto weight=std::clamp(t,0.f,1.f);
       *(float*)(e+0x394)=weight;*(float*)(e+0x398)=broken;
       for(int j=0;j<4;j++){
        auto dark=((float*)(e+0x370))[j],light=((float*)(e+0x380))[j];
        auto color=dark+weight*(light-dark);
        ((float*)(e+0x1b0))[j]=color+broken*(((float*)0x14076e8e0ull)[j]-color);
       }
       break;
      }
      case 0x1406963d8ull:e[0x330]=a;break;
      case 0x1406a9808ull:e[0x348]=a;*(float*)(e+0x338)=t;break;
      default:throw std::runtime_error("Unexpected mechanism type");
     }
    }
    for(auto ptr:entities)*(float*)((uint8_t*)ptr+0x284)=0;
    const auto countTransmuted=in.word();
    if(countTransmuted>byId.size())throw std::runtime_error("Invalid transmutation count");
    for(unsigned i=0;i<countTransmuted;i++){auto id=in.word();*(float*)(byId.at(id)+0x284)=readFloat();}
    effects.read(in);
   }
   // The original Bard pass in 0x1403765a0 reads this 0x34-byte record array.
   *(uint64_t*)(storage.data()+0x6830)=bardRanges.size();
   *(void**)(storage.data()+0x6838)=bardRanges.data();
   *(uint64_t*)(storage.data()+0x6840)=bardRanges.size();
   *(uint64_t*)(storage.data()+0x41f0)=0;*(uint64_t*)(storage.data()+0x4228)=1;*(void**)(storage.data()+0x4230)=&activeFamily;
  }
 }

};
