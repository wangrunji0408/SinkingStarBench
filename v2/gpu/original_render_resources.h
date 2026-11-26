#pragma once
#include "original_runtime.h"
inline uint32_t renderDimension(const char* key,uint32_t fallback){
 const char* value=getenv(key);
 if(!value)return fallback;
 auto n=std::stoul(value);
 if(n<16||n>8192)throw std::runtime_error("Render dimensions must be between 16 and 8192");
 return uint32_t(n);
}
inline uint32_t renderWidth(){static const auto n=renderDimension("SINKING_RENDER_WIDTH",1280);return n;}
inline uint32_t renderHeight(){static const auto n=renderDimension("SINKING_RENDER_HEIGHT",720);return n;}
// Pinned build: enter only init_graphics' GPU-resource tail, after its window,
// swapchain, display settings and UI initialization. Preserve its own prologue/epilogue.
extern "C" void __attribute__((ms_abi)) originalResourceTailSetup(uint8_t*stack){
 if(*(uint8_t*)0x140805039==8)*(uint8_t*)0x140805039=*(uint8_t*)0x140804a28;
 memset(stack,0,0x3210);*(uint64_t*)(stack+0x50)=renderWidth();*(uint64_t*)(stack+0x48)=renderHeight();
 unsigned mode=getenv("SINKING_GRAPHICS_MODE")?std::stoul(getenv("SINKING_GRAPHICS_MODE")):3;if(mode<1||mode>4)throw std::runtime_error("Graphics mode must be 1..4");
 using Settings=void (__attribute__((ms_abi)) *)(void*,uint64_t,uint64_t,uint64_t,uint64_t,void*);((Settings)0x1401d6940ull)((void*)0x140807260,mode,1,renderWidth(),renderHeight(),(void*)0x1410903c0);
 fprintf(stderr,"Original GPU resource tail (%u x %u)\n",renderWidth(),renderHeight());
}
extern "C" void __attribute__((naked,ms_abi)) originalResourceTailEntry(){
 __asm__ volatile("movq %rsp, %rcx\n subq $32, %rsp\n callq _originalResourceTailSetup\n addq $32, %rsp\n leaq 0x8b0(%rsp), %rsi\n movabsq $0x1401f1355, %rax\n jmpq *%rax\n");
}
using OriginalTextureCreate=void (__attribute__((ms_abi)) *)(void*,uint8_t*,void*,void*,void*);
inline OriginalTextureCreate originalTextureCreate;
inline std::vector<uint8_t*> shadowReadbacks;
inline std::map<std::string,uint8_t*> debugTargets;
inline void __attribute__((ms_abi)) tracedTextureCreate(void*ctx,uint8_t*t,void*data,void*clear,void*out){fprintf(stderr,"Original texture creation: %.*s %u x %u x %u format=%u flags=%x\n",int(*(uint64_t*)t),*(char**)(t+8),*(uint32_t*)(t+0x38),*(uint32_t*)(t+0x3c),*(uint32_t*)(t+0x40),*(uint16_t*)(t+0x48),*(uint16_t*)(t+0x4a));originalTextureCreate(ctx,t,data,clear,out);if(getenv("SINKING_DUMP_TARGETS"))debugTargets[std::string(*(char**)(t+8),*(uint64_t*)t)]=t;if(std::string(*(char**)(t+8),*(uint64_t*)t)=="shadow_target"||std::string(*(char**)(t+8),*(uint64_t*)t)=="evsm_shadow_map"||std::string(*(char**)(t+8),*(uint64_t*)t)=="atmosphere_transmittance")shadowReadbacks.push_back(t);}
inline void traceTextureCreation(OriginalRuntime&r){auto code=(uint8_t*)mmap(nullptr,4096,PROT_READ|PROT_WRITE,MAP_PRIVATE|MAP_ANON,-1,0);if(code==MAP_FAILED)abort();memcpy(code,(void*)0x140163a20,17);uint8_t jump[]={0xff,0x25,0,0,0,0};memcpy(code+17,jump,6);*(uint64_t*)(code+23)=0x140163a31; mprotect(code,4096,PROT_READ|PROT_EXEC);originalTextureCreate=(OriginalTextureCreate)code;r.image.redirect(0x140163a20,(void*)tracedTextureCreate);}
inline void createOriginalRenderResources(OriginalRuntime&r){traceTextureCreation(r);r.image.redirect(0x1401eeb66,(void*)originalResourceTailEntry);using F=void (__attribute__((ms_abi)) *)();r.image.function<F>(0x1401eeb10)();fprintf(stderr,"Original GPU render resources completed\n");}
