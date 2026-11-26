#pragma once
#include "original_shader_catalog.h"
#include "original_constant_buffer.h"
struct MaterialCatalog {
 struct Value {Bytes name,raw,text;uint32_t kind;};
 struct Material {Bytes name,shader,storage,source;std::vector<Value> values;uint32_t flags=0;std::streamoff offset=-1,end=-1;bool initialized=false;};
 Bytes table=Bytes(65536*32);uint64_t loaded=0;std::map<std::string,std::unique_ptr<Material>> assets;OriginalRuntime &runtime;inline static MaterialCatalog *active;
 Reader in;
 void readMaterial(Material& m){
  m.shader=in.blob();m.flags=in.word();auto n=in.word();m.values.reserve(n);
  for(unsigned j=0;j<n;j++){Value v;v.name=in.blob();v.kind=in.word();v.raw=in.blob();v.text=in.blob();if(v.raw.size()!=16)throw std::runtime_error("Invalid material value");m.values.push_back(std::move(v));}
 }
 void hydrate(Material& m){
  if(m.offset<0)return;
  in.f.seekg(m.offset);readMaterial(m);
  if(in.f.tellg()!=m.end)throw std::runtime_error("Invalid material record extent");
  m.offset=-1;
 }
 MaterialCatalog(const char*path,OriginalRuntime&r,Host&h):runtime(r),in(path){
  active=this;*(uint64_t*)0x1410d7a38=65536;*(uint64_t*)0x1410d7a58=65536;*(void**)0x1410d7a60=this->table.data();materialGPU=&h;
  r.image.redirect(0x1401801e0,(void*)constantBuffer);r.image.redirect(0x1401915c0,(void*)lookup);
  auto count=in.word();bool indexed=count==0x3254414d;if(indexed)count=in.word();
  for(unsigned i=0;i<count;i++){
   auto m=std::make_unique<Material>();m->name=in.blob();
   if(indexed){auto size=in.word();m->offset=in.f.tellg();m->end=m->offset+size;in.f.seekg(m->end);}
   else readMaterial(*m);
   assets.emplace(std::string(m->name.begin(),m->name.end()),std::move(m));
  }
 }
 static void __attribute__((ms_abi)) lookup(void*ctx,uint64_t*name,uint8_t,void**out,uint8_t*ok){*out=active->load(ctx,std::string((char*)name[1],name[0]));*ok=*out!=nullptr;}
 void* load(void*ctx,const std::string&name){auto it=assets.find(name);if(it==assets.end()){fprintf(stderr,"Original material fallback to default: %s\n",name.c_str());auto fallback=assets.find("default");if(fallback==assets.end())throw std::runtime_error("Original default material unavailable");hydrate(*fallback->second);auto m=std::make_unique<Material>();m->name=Bytes(name.begin(),name.end());m->shader=fallback->second->shader;m->flags=fallback->second->flags;m->values=fallback->second->values;it=assets.emplace(name,std::move(m)).first;}auto&m=*it->second;if(m.initialized)return m.storage.data();hydrate(m);m.initialized=true;m.storage.resize(272);m.source.resize(144);using Ctor=void (__attribute__((ms_abi)) *)(void*);runtime.image.function<Ctor>(0x14007ab20)(m.source.data());
  auto put=[](Bytes&b,size_t o,auto v){memcpy(b.data()+o,&v,sizeof v);};put(m.storage,0,uint64_t(m.name.size()));put(m.storage,8,(uintptr_t)m.name.data());put(m.storage,0x40,(uintptr_t)(m.storage.data()+0x48));put(m.storage,0x50,(uintptr_t)m.source.data());put(m.source,0x38,uint64_t(m.shader.size()));put(m.source,0x40,(uintptr_t)m.shader.data());put(m.source,0x48,m.flags);
  uint64_t cap=32;while(cap<m.values.size()*2)cap*=2;void*allocation=nullptr;OriginalRuntime::allocate(ctx,0,cap*72,0,nullptr,0,&allocation);auto table=(uint8_t*)allocation;put(m.source,0x58,uint64_t(m.values.size()));put(m.source,0x60,cap);put(m.source,0x80,cap);put(m.source,0x88,(uintptr_t)table);
  for(auto&v:m.values){uint32_t hash=0x1505;for(auto c:v.name)hash=(hash^c)*0x1b3;if(hash<2)hash|=2;auto slot=hash&(cap-1);uint32_t step=1;while(*(uint32_t*)(table+slot*72))slot=(slot+step++)&(cap-1);auto p=table+slot*72;memcpy(p,&hash,4);auto owned=[&](const Bytes&b){void*p=nullptr;OriginalRuntime::allocate(ctx,0,b.size(),0,nullptr,0,&p);memcpy(p,b.data(),b.size());return (uintptr_t)p;};uint64_t fields[]={v.name.size(),owned(v.name),v.kind};memcpy(p+8,fields,sizeof fields);p[32]=p[33]=1;memcpy(p+40,v.raw.data(),16);uint64_t str[]={v.text.size(),owned(v.text)};memcpy(p+56,str,16);}
  fprintf(stderr,"Original material initialization: %s\n",name.c_str());uint8_t ok=0;using F=void (__attribute__((ms_abi)) *)(void*,void*,uint8_t*);runtime.image.function<F>(0x1401f79d0)(ctx,m.storage.data(),&ok);if(!ok){fprintf(stderr,"Material initialization failed\n");_Exit(4);}m.storage[0x20]=1;uint32_t hash=0x1505;for(auto c:m.name)hash=(hash^c)*0x1b3;if(hash<2)hash|=2;uint32_t slot=hash&65535,step=1;while(*(uint32_t*)(this->table.data()+slot*32))slot=(slot+step++)&65535;auto p=this->table.data()+slot*32;memcpy(p,&hash,4);uint64_t entry[]={m.name.size(),(uintptr_t)m.name.data(),(uintptr_t)m.storage.data()};memcpy(p+8,entry,24);*(uint64_t*)0x1410d7a30=++loaded;return m.storage.data();
 }
};
