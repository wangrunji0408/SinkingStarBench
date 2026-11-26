#pragma once
#include <unordered_map>
// r68307 constructs shader metadata/root signatures separately from its Metal
// pipeline job. Keep the native inputs and run that same job on the first draw.
struct DeferredPipelines {
 inline static bool enabled=false;
 inline static Host* host=nullptr;
 inline static size_t registered=0;
 inline static std::unordered_map<void*,Bytes> pending;
 using Job=void (__attribute__((ms_abi)) *)(void*,uint64_t,uint64_t,void**,uint64_t*);
 static bool defer(void* ctx,void** shader){
  auto s=(uint8_t*)*shader;
  if(!enabled||*(uintptr_t*)(s+0x167a0)||(!*(uintptr_t*)(s+0x16780)&&!*(uintptr_t*)(s+0x167b0)))return false;
  auto [it,added]=pending.try_emplace(s,4096);
  if(added){memcpy(it->second.data(),ctx,0x300);++registered;}
  return true;
 }
 static void ensure(void** shader){
  auto it=pending.find(*shader);if(it==pending.end())return;
  uint64_t done=0;((Job)0x140229540ull)(it->second.data(),0,0,shader,&done);
  auto p=(ID3D12PipelineState**)((char*)*shader+0x167e0);
  if(!done||!p[0])throw std::runtime_error("Original graphics pipeline unavailable");
  host->keep(p[0]);pending.erase(it);
 }
 using Draw=void (__attribute__((ms_abi)) *)(void*,uint64_t*);
 inline static Draw packet=nullptr,geometry=nullptr,immediate=nullptr;
 static void __attribute__((ms_abi)) drawPacket(void* ctx,uint64_t* p){
  // Packet kinds 0 and 2 delegate to the geometry and immediate handlers.
  if(p[0]==1||p[0]==3)ensure((void**)p[1]);packet(ctx,p);
 }
 static void __attribute__((ms_abi)) drawGeometry(void* ctx,uint64_t* p){ensure((void**)p[0]);geometry(ctx,p);}
 static void __attribute__((ms_abi)) drawImmediate(void* ctx,uint64_t* p){ensure((void**)p[0]);immediate(ctx,p);}
 static void install(OriginalImage& image,Host& h){
  host=&h;enabled=true;
  auto forward=[&](uintptr_t address,size_t size,Draw hook){
   auto* code=(uint8_t*)mmap(nullptr,4096,PROT_READ|PROT_WRITE,MAP_PRIVATE|MAP_ANON,-1,0);
   if(code==MAP_FAILED)throw std::runtime_error("Cannot allocate draw adapter");
   memcpy(code,(void*)address,size);
   const uint8_t jump[]={0xff,0x25,0,0,0,0};memcpy(code+size,jump,sizeof jump);
   uintptr_t next=address+size;memcpy(code+size+6,&next,8);
   if(mprotect(code,4096,PROT_READ|PROT_EXEC))throw std::runtime_error("Cannot finalize draw adapter");
   image.redirect(address,(void*)hook);return (Draw)code;
  };
  // These whole-instruction prologues of the hash-checked r68307 contain no
  // PC-relative operands. Direct draws also enter the latter two routines.
  packet=forward(0x140094f50,15,drawPacket);geometry=forward(0x1400c7520,16,drawGeometry);immediate=forward(0x140119760,17,drawImmediate);
 }
};
