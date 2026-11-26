#pragma once
#include <set>
// Late-bound original particle group requests from the replay. Template storage
// is separate from the render world so authored emitters never appear on screen.
struct OriginalEffects {
 struct Event {uint32_t id,attachment,group,mode,destination;double time;float position[3],orientation[4];std::string name;};
 struct Live {uint32_t uid;float duration,attachmentOffset[3];};
 Bytes library=Bytes(32640);
 std::vector<Event> events;
 std::map<uint32_t,std::vector<Live>> live;
 double clock=0;
 double lastClock=-1;
 void read(Reader&in){
  if(in.word()!=0x46585431)throw std::runtime_error("Invalid effects snapshot");
  in.f.read((char*)&clock,8);auto n=in.word();events.clear();
  if(!std::isfinite(clock)||clock<0||n>16384)throw std::runtime_error("Invalid effects clock/count");
  for(unsigned i=0;i<n;i++){
   Event e{};e.id=in.word();in.f.read((char*)&e.time,8);
   in.f.read((char*)e.position,12);in.f.read((char*)e.orientation,16);
   e.attachment=in.word();e.group=in.word();e.mode=in.word();e.destination=in.word();auto len=in.word();
   if(!in.f||!len||len>128||!std::isfinite(e.time))throw std::runtime_error("Invalid particle event");
   e.name.resize(len);in.f.read(e.name.data(),len);
   if(!in.f)throw std::runtime_error("Truncated particle name");
   events.push_back(std::move(e));
  }
 }
 void load(const char*path,OriginalRuntime&r,Bytes&ctx){
  if(!std::ifstream(path))return;
  using Ctor=void (__attribute__((ms_abi)) *)(void*);
  using Create=void (__attribute__((ms_abi)) *)(void*,void*,uint64_t,uint32_t,void**);
  using Apply=void (__attribute__((ms_abi)) *)(void*,uint64_t,void*,uint64_t,uint64_t*,uint64_t,uint64_t,uint64_t,uint8_t,uint8_t*);
  using Register=void (__attribute__((ms_abi)) *)(void*,void*,void*);
  auto w=library.data();r.image.function<Ctor>(0x14007cff0)(w);*(uint8_t**)(w+0x3ef0)=w;w[0x3ee8]=1;
  Reader in(path);uint64_t base=0;in.f.read((char*)&base,8);auto n=in.word();
  if(n>4096)throw std::runtime_error("Invalid particle library count");
  for(unsigned i=0;i<n;i++){
   auto uid=in.word();uint64_t type=0,version=0;in.f.read((char*)&type,8);in.f.read((char*)&version,8);auto body=in.blob();
   if(type!=0x140694510ull&&type!=0x140682ba8ull)throw std::runtime_error("Invalid particle library type");
   void* p=nullptr;r.image.function<Create>(0x140409400)(ctx.data(),w,type,uid,&p);
   auto e=(uint8_t*)p;const auto handle=*(uint64_t*)(e+0xf8),ctor=*(uint64_t*)(type+0x78);
   if(ctor)r.image.function<Ctor>(ctor)(e);
   *(uint64_t*)(e+0xf8)=handle;*(uint8_t**)(e+0xf0)=w;*(uint32_t*)(e+0x100)=uid;*(uint64_t*)(e+0x108)=type;
   *(int32_t*)(e+0x110)=*(int32_t*)(e+0x114)=-1;
   uint64_t data[]={body.size(),(uintptr_t)body.data()};uint8_t ok=0;
   r.image.function<Apply>(0x1402386b0)(ctx.data(),type,e,0,data,base,version,0,1,&ok);
   if(!ok)throw std::runtime_error("Invalid original particle entity");
   memcpy(e+0xa0,e,12);memcpy(e+0x1d0,e,12);
   r.image.function<Register>(0x140326f40)(ctx.data(),w,e);
  }
  fprintf(stderr,"Particle library groups=%llu sources=%llu\n",*(unsigned long long*)(w+0x8f8),*(unsigned long long*)(w+0xc18));
  *(void**)0x140b6cdb8=w;
 }
 float frameStep(bool movie){
  const auto prior=lastClock;lastClock=clock;
  return movie&&prior>=0?float(std::clamp(clock-prior,0.,4./30)):1.f/30;
 }
 void resetParticles(OriginalRuntime&r,Bytes&ctx,Bytes&world){
  if(live.empty())return;
  std::set<uint32_t> owned;
  for(const auto& e:live)for(const auto& l:e.second)owned.insert(l.uid);
  auto w=world.data();
  using Remove=void (__attribute__((ms_abi)) *)(void*,void*,uint8_t);
  for(uint64_t i=*(uint64_t*)(w+0xc18);i-->0;){
   auto p=(*(uint8_t***)(w+0xc20))[i];
   if(owned.count(*(uint32_t*)(p+0x100)))r.image.function<Remove>(0x140325cc0)(ctx.data(),p,1);
  }
  live.clear();
 }
 void draw(OriginalRuntime&r,Bytes&ctx,Bytes&world,const std::vector<void*>&entities,double displayedClock,float dt){
  if(events.empty()&&live.empty())return;
  auto w=world.data();
  using Spawn=void (__attribute__((ms_abi)) *)(void*,void*,void*,const float*,const float*,const uint64_t*,uint8_t,uint64_t*,void**);
  auto resolve=[&](uint32_t id)->uint8_t*{
   if(id)for(auto p:entities){auto e=(uint8_t*)p;if(*(uint32_t*)(e+0x100)==id)return e;}
   return nullptr;
  };
  std::map<uint32_t,const Event*> current;
  for(const auto&e:events){
   current[e.id]=&e;
   if(e.time>displayedClock||live.count(e.id))continue;
   auto owner=resolve(e.attachment);
   uint64_t name[]={e.name.size(),(uintptr_t)e.name.data()},sources[5]={};void* group=nullptr;
   const auto previous=*(uint64_t*)(w+0xc18);
   if(e.mode==1&&owner&&*(uint64_t*)(owner+0x108)==0x1406920a8ull){
    // Leave the placement, orientation and authored gate color to the original gate effect.
    using Gate=void (__attribute__((ms_abi)) *)(void*,void*,uint8_t);
    r.image.function<Gate>(0x14025d250)(ctx.data(),owner,e.name.find("GateOpen")==0);
    sources[0]=*(uint64_t*)(w+0xc18)-previous;
    if(sources[0])sources[1]=(uintptr_t)(*(uint8_t***)(w+0xc20)+previous);
   }else r.image.function<Spawn>(0x140311a70)(ctx.data(),w,nullptr,e.position,e.orientation,name,e.group,sources,&group);
   if(sources[0]>4096)throw std::runtime_error("Invalid spawned particle list");
   auto&active=live[e.id];
   for(uint64_t i=0;i<sources[0];i++){
    auto p=((uint8_t**)sources[1])[i];
    const auto lifetime=std::max(*(float*)(p+0x3a8),*(float*)(p+0x3ac));
    const auto duration=std::max(0.f,*(float*)(p+0x340))+std::max(0.f,*(float*)(p+0x33c))+std::max(0.f,lifetime);
    if(e.name=="GateCloseDust")p[0x614]=1;
    if(e.mode==3){
     *(uint64_t*)(p+0xce0)=1; // Original BardRadialParticles render mode.
     *(uint64_t*)(p+0xcd8)=e.attachment;
    }
    Live l{*(uint32_t*)(p+0x100),duration,{}};
    const auto age=std::max(0.,displayedClock-e.time-dt);
    *(float*)(p+0x4e0)=float(age);
    p[0x4e8]=1;
    if(p[0x7a5])*(uint64_t*)(p+0xd08)=*(uint64_t*)(p+0x7a8);
    *(float*)(p+0x510)=1;
    for(int j=0;j<3;j++)l.attachmentOffset[j]=((float*)p)[j]-e.position[j];
    active.push_back(l);
    using Register=void (__attribute__((ms_abi)) *)(void*,void*,void*,uint8_t);
    r.image.function<Register>(0x1403ad390)(ctx.data(),w,p,1);
    if(getenv("SINKING_TRACE_PARTICLES"))fprintf(stderr,"Spawned %s at %g,%g,%g duration=%g\n",e.name.c_str(),((float*)p)[0],((float*)p)[1],((float*)p)[2],duration);
   }
  }
  std::map<uint32_t,uint8_t*> sources;
  for(uint64_t j=0;j<*(uint64_t*)(w+0xc18);j++){
   auto p=(*(uint8_t***)(w+0xc20))[j];sources[*(uint32_t*)(p+0x100)]=p;
  }
  for(auto it=live.begin();it!=live.end();){
   const auto entry=current.find(it->first);const auto event=entry!=current.end()?entry->second:nullptr;
   for(auto& l:it->second){
    auto found=sources.find(l.uid);if(found==sources.end())continue;auto p=found->second;
    const float age=event?std::max(0.,displayedClock-event->time):0;
    if(!event||(event->mode<2&&age>l.duration)){
     using Remove=void (__attribute__((ms_abi)) *)(void*,void*,uint8_t);
     r.image.function<Remove>(0x140325cc0)(ctx.data(),p,1);
     continue;
    }
   if(event->mode>=2){
    // Keep the authored emitter producing without extending individual particle lifetimes.
    const auto emission=std::max(0.f,*(float*)(p+0x33c));
    *(float*)(p+0x33c)=std::max(emission,age+1.f);
    auto target=resolve(event->destination);
    const float travel=target?std::fmod(age,0.85f)/0.85f:0;
    for(int j=0;j<3;j++){
     const float start=event->position[j];
     const float end=target?((float*)(target+0x1d0))[j]+(j==2?1.45f:0.f):start;
     ((float*)p)[j]=l.attachmentOffset[j]+start+travel*(end-start)+(j==2&&target?0.35f*std::sin(travel*3.14159265f):0);
    }
   }
   memcpy(p+0xa0,p,12);memcpy(p+0x1d0,p,12);memcpy(p+0x1dc,p+0x18,16);memcpy(p+0x1ec,p+0xc,12);
   if(getenv("SINKING_TRACE_PARTICLES"))fprintf(stderr,"Live particle uid=%u worldSources=%llu age=%g capacity=%llu fade=%g rate=%g\n",*(uint32_t*)(p+0x100),*(unsigned long long*)(w+0xc18),age,*(unsigned long long*)(p+0xcc8),*(float*)(p+0x510),*(float*)(p+0x338));
   }
   if(!event)it=live.erase(it);else ++it;
  }
 }
};
