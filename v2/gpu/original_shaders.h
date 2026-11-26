#pragma once
#include "original_texture_catalog.h"
#include <memory>
#include <map>
struct OriginalShader {
 Bytes name,raw,states,storage;void *handle;
 OriginalShader(Reader &in,OriginalRuntime &runtime,Host &host):name(in.blob()),raw(in.blob()),states(in.blob()),storage(0x16800),handle(storage.data()){initialize(runtime,host);}
 OriginalShader(Bytes n,Bytes r,Bytes s,OriginalRuntime &runtime,Host &host):name(std::move(n)),raw(std::move(r)),states(std::move(s)),storage(0x16800),handle(storage.data()){initialize(runtime,host);}
 void initialize(OriginalRuntime &runtime,Host &host){
  std::string assetName(name.begin(),name.end());auto dash=assetName.rfind('-');if(dash!=std::string::npos)*(uint32_t*)(storage.data()+0x40)=std::stoul(assetName.substr(dash+1));uint64_t n[]={name.size(),(uintptr_t)name.data()};memcpy(storage.data(),n,sizeof n);Bytes ctx;runtime.context(ctx);uint64_t data[]={raw.size(),(uintptr_t)raw.data()};
  using F=void (__attribute__((ms_abi)) *)(void*,void**,uint64_t*);runtime.image.function<F>(0x1401aa0e0ull)(ctx.data(),&handle,data);
  if(states.size()!=46||memcmp(storage.data()+0x62,states.data(),46))throw std::runtime_error("Original loader state mismatch");
  auto root=*(ID3D12RootSignature**)(storage.data()+0x167c0);auto pipeline=*(ID3D12PipelineState**)(storage.data()+0x167e0);auto compute=*(ID3D12PipelineState**)(storage.data()+0x167d8);bool hasProgram=*(uint64_t*)(storage.data()+0x16780)||*(uint64_t*)(storage.data()+0x167a0);if(hasProgram&&(!root||(!pipeline&&!compute&&!DeferredPipelines::pending.count(handle))))throw std::runtime_error("Original shader pipeline missing");if(root)host.keep(root);if(pipeline)host.keep(pipeline);if(compute)host.keep(compute);
 }
};
struct OriginalShaders {
 std::map<std::string,std::unique_ptr<OriginalShader>> assets;
 OriginalShaders(const char*path,OriginalRuntime &runtime,Host &host){runtime.image.redirect(0x14022ac50,(void*)OriginalRuntime::shaderJob);Reader in(path);auto count=in.word();if(count>3000)throw std::runtime_error("Invalid shader count");for(unsigned i=0;i<count;i++){auto p=std::make_unique<OriginalShader>(in,runtime,host);std::string name(p->name.begin(),p->name.end());fprintf(stderr,"Loaded original shader: %s\n",name.c_str());assets.emplace(name,std::move(p));}}
};
