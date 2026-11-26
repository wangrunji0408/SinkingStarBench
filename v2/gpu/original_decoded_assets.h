#pragma once
#include "original_runtime.h"
#include <cstdlib>
static std::string game_data_pc(){const char*env=std::getenv("GAME_ROOT");if(env)return std::string(env)+"/data-pc/";const char*home=std::getenv("HOME");return std::string(home?home:"")+"/Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam/steamapps/common/Order of the Sinking Star/data-pc/";}
// Replace only asset file IO/decompression. The original texture parser and render calls remain native.
struct OriginalDecodedAssets {
 inline static OriginalDecodedAssets*active;std::string directory;
 OriginalDecodedAssets(OriginalRuntime&r,std::string path):directory(std::move(path)){active=this;r.image.redirect(0x140186750,(void*)load);}
 static void __attribute__((ms_abi)) load(void*ctx,uint64_t*name,uint16_t type,uint64_t flags,void*,uint64_t*out,uint8_t*ok){
  std::string n((char*)name[1],name[0]);if(type!=0||flags!=8||n.find('/')!=std::string::npos)throw std::runtime_error("Unsupported decoded asset request: "+n);
  auto path=game_data_pc()+n+".texture";if(!std::ifstream(path)){if(n.size()<12||n.compare(n.size()-12,12,"-probe_array")!=0)throw std::runtime_error("Missing required texture: "+n);memset(out,0,6*sizeof(uint64_t));out[2]=~0ull;*ok=0;return;}auto packed=sourceBytes(path);auto payload=gamePayload(packed);auto size=40+payload.size();void*data;OriginalRuntime::allocate(ctx,0,size,0,nullptr,0,&data);memcpy(data,packed.data(),40);memset((char*)data+4,0,2);memcpy((char*)data+40,payload.data(),payload.size());out[0]=size;out[1]=(uintptr_t)data;out[2]=~0ull;out[3]=out[4]=out[5]=0;*ok=1;fprintf(stderr,"Original decoded texture load: %s (%zu bytes)\n",n.c_str(),size);
 }
};
