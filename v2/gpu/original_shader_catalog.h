#pragma once
#include "original_shaders.h"
struct Catalog {
 struct Asset {uint64_t offset,size;Bytes states;std::unique_ptr<OriginalShader> shader;};
 std::map<std::string,Asset> assets;OriginalRuntime &runtime;Host &host;inline static Catalog *active;
 Catalog(const char*path,OriginalRuntime&r,Host&h):runtime(r),host(h){active=this;std::ifstream f(path);std::string line;while(std::getline(f,line)){auto a=line.find('\t'),b=line.find('\t',a+1);if(a==std::string::npos||b==std::string::npos)throw std::runtime_error("Bad catalog");Asset asset;asset.offset=std::stoull(line.substr(a+1,b-a-1));asset.size=std::stoull(line.substr(b+1));assets.emplace(line.substr(0,a),std::move(asset));}runtime.image.redirect(0x140198d70,(void*)lookup);runtime.image.redirect(0x14022ac50,(void*)OriginalRuntime::shaderJob);}
 static void __attribute__((ms_abi)) lookup(void*,uint64_t*name,uint8_t*flags,void***out){
  std::string n((char*)name[1],name[0]);auto extra=(uint64_t*)(flags+8);std::string key="data-pc/"+n+"-"+std::string((char*)extra[1],extra[0])+"-"+std::to_string(*(uint32_t*)flags)+".shader";
  fprintf(stderr,"Global shader request: %s\n",key.c_str());if(n=="beam")fprintf(stderr,"Beam shader reference output=%p\n",out);auto it=active->assets.find(key);if(it==active->assets.end()){fprintf(stderr,"Missing shader asset\n");abort();}auto &a=it->second;
  if(!a.shader){Bytes raw=gamePayload(sourceBytes(std::string(getenv("GAME_ROOT"))+"/data/shaders.package",a.offset,a.size));if(raw.size()<57)throw std::runtime_error("Truncated shader");a.states.assign(raw.begin()+11,raw.begin()+57);a.shader=std::make_unique<OriginalShader>(Bytes(key.begin(),key.end()),std::move(raw),a.states,active->runtime,active->host);}*out=&a.shader->handle;
 }
};
